use {
    super::{
        Collector,
        blank_nodes::BlankNodes,
        hashing_reader::HashingReader,
        source::{FileInput, MAX_RDF_BYTES},
    },
    crate::publication::writer::ContributionStream,
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
pub struct RdfFile(FileInput);

impl RdfFile {
    /// Configure one RDF source. Unsupported formats are rejected during
    /// collection.
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self(FileInput::new(project, collector, path)?))
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
        Ok(format!("rdf-v1:{}", hex::encode(hash.finalize())))
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
        while let Some(quad) = parser.next().await {
            let quad = match quad {
                Ok(quad) => quad,
                Err(error) => {
                    return Err(error.into());
                },
            };
            stream.write(blanks.normalize(quad)).await?;
        }
        drop(parser);
        if raw_revision != observed.revision() {
            anyhow::bail!("RDF source changed during collection");
        }
        Ok(revision)
    }
}
