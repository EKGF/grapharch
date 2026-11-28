use {
    super::{
        Collector,
        blank_nodes::BlankNodes,
        hashing_reader::HashingReader,
        source::{FileInput, MAX_RDF_BYTES},
    },
    crate::publication::{rdf_target::SourceText, writer::ContributionStream},
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdfio::{RdfFormat, RdfParser},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
    tokio::{
        fs::File,
        io::{AsyncReadExt, BufReader},
    },
};

/// Imports one RDF document, preserving its named graphs and assertions.
pub struct RdfFile(FileInput, bool);

impl RdfFile {
    /// Configure one RDF source. Unsupported formats are rejected during
    /// collection.
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self(
            FileInput::new(project, collector, path)?,
            false,
        ))
    }

    /// Also retain the source's exact text, which an RDF renderer target
    /// that names this collector use publishes.
    pub(crate) fn retaining_text(mut self, retain: bool) -> Self {
        self.1 = retain;
        self
    }

    async fn retain(
        &self,
        stream: &mut ContributionStream,
        format: RdfFormat,
        base: &str,
        revision: &str,
    ) -> Result<()> {
        let bytes = tokio::fs::read(&self.0.path).await?;
        ensure!(
            bytes.len() as u64 <= MAX_RDF_BYTES &&
                format!("rdf-v2:{}", hex::encode(Sha256::digest(&bytes))) ==
                    revision,
            "RDF source changed during collection"
        );
        let text = String::from_utf8(bytes).map_err(|_| {
            anyhow::anyhow!("a published RDF source must be UTF-8 text")
        })?;
        let processor = oxrdf::NamedNode::new(self.0.collector.as_str())?;
        for quad in
            SourceText::new(text, format.media_type(), base)?.quads(&processor)
        {
            stream.write(quad).await?;
        }
        Ok(())
    }

    async fn digest(&self) -> Result<String> {
        let mut file = File::open(&self.0.path).await?;
        let mut buffer = [0; 16384];
        let mut hash = Sha256::new();
        let mut size = 0;
        loop {
            let count = file.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            size += count as u64;
            ensure!(
                size <= MAX_RDF_BYTES,
                "RDF source exceeds configured byte limit"
            );
            hash.update(&buffer[..count]);
        }
        Ok(format!("rdf-v2:{}", hex::encode(hash.finalize())))
    }
}

#[async_trait]
impl Collector for RdfFile {
    fn identity(&self) -> Result<super::SourceIdentity<'_>> {
        self.0.identity()
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let extension = self
            .0
            .path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("missing RDF format extension"))?;
        let format = RdfFormat::from_extension(extension).ok_or_else(|| {
            anyhow::anyhow!("unsupported RDF format: {extension}")
        })?;
        let raw_revision = self.digest().await?;
        let base = url::Url::from_file_path(
            tokio::fs::canonicalize(&self.0.path).await?,
        )
        .map_err(|_| anyhow::anyhow!("invalid source file URL"))?;
        let revision = format!("{raw_revision}:base:{base}");
        let file = File::open(&self.0.path).await?;
        let mut observed =
            HashingReader::new(BufReader::new(file), MAX_RDF_BYTES);
        let mut blanks = BlankNodes::new(
            self.0.project.as_str(),
            self.0.collector.as_str(),
            self.0.source()?,
            &revision,
        );
        let mut parser = RdfParser::from_format(format)
            .with_base_iri(base.as_str())?
            .for_tokio_async_reader(&mut observed);
        let mut modules = super::module_hints::ModuleHints::default();
        while let Some(quad) = parser.next().await {
            let quad = match quad {
                Ok(quad) => quad,
                Err(error) => {
                    return Err(error.into());
                },
            };
            let quad = blanks.normalize(quad);
            modules.observe(&quad)?;
            if quad.predicate.as_str() ==
                crate::publication::document::description::MARKDOWN &&
                let oxrdf::Term::Literal(body) = &quad.object
            {
                for reference in super::reference_facts::describe_description(
                    &self.0,
                    &quad.subject.to_string(),
                    body.value(),
                ) {
                    stream.write(reference).await?;
                }
            }
            stream.write(quad).await?;
        }
        drop(parser);
        for quad in modules.finish()? {
            stream.write(quad).await?;
        }
        if raw_revision != observed.revision() {
            anyhow::bail!("RDF source changed during collection");
        }
        if self.1 {
            self.retain(stream, format, base.as_str(), &raw_revision)
                .await?;
        }
        Ok(revision)
    }
}
