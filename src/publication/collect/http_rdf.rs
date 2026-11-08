//! One explicit HTTP RDF document, retained as native assertions with
//! provenance.
use {
    super::{
        Collector,
        SourceIdentity,
        blank_nodes::BlankNodes,
        module_hints::ModuleHints,
    },
    crate::publication::writer::ContributionStream,
    anyhow::{Result, ensure},
    async_trait::async_trait,
    futures::StreamExt,
    oxrdf::NamedNode,
    oxrdfio::{RdfFormat, RdfParser},
    sha2::{Digest, Sha256},
    std::{io::SeekFrom, time::Duration},
    tokio::io::{AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter},
};

const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Collect one explicitly typed RDF document from an HTTP(S) URL.
pub struct RdfUrl {
    project:   NamedNode,
    processor: NamedNode,
    url:       url::Url,
    format:    RdfFormat,
}

fn checked_url(value: &str) -> Result<url::Url> {
    let url = url::Url::parse(value)?;
    ensure!(
        matches!(url.scheme(), "http" | "https") &&
            url.host_str().is_some() &&
            url.username().is_empty() &&
            url.password().is_none() &&
            url.fragment().is_none(),
        "RDF URL requires credential-free HTTP(S) without a fragment"
    );
    Ok(url)
}

impl RdfUrl {
    /// Bind the parser format; an RDF-specific transport type must agree.
    pub fn new(
        project: &str,
        processor: &str,
        source: &str,
        mime: &str,
    ) -> Result<Self> {
        let format = RdfFormat::from_media_type(mime).ok_or_else(|| {
            anyhow::anyhow!("unsupported RDF URL media type {mime}")
        })?;
        Ok(Self {
            project: NamedNode::new(project)?,
            processor: NamedNode::new(processor)?,
            url: checked_url(source)?,
            format,
        })
    }
}

#[async_trait]
impl Collector for RdfUrl {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: self.processor.as_str(),
            source:    self.url.as_str(),
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3 {
                    attempt.error("RDF URL redirect limit exceeded")
                } else if checked_url(attempt.url().as_str()).is_err() {
                    attempt.error("unsafe RDF URL redirect")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        let response = client
            .get(self.url.clone())
            .header(reqwest::header::ACCEPT, self.format.media_type())
            .send()
            .await?
            .error_for_status()?;
        ensure!(
            response
                .content_length()
                .is_none_or(|size| size <= MAX_BYTES),
            "RDF URL exceeds 8 MiB byte limit"
        );
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .ok_or_else(|| {
                anyhow::anyhow!("RDF URL response requires Content-Type")
            })?
            .to_str()?;
        ensure!(
            RdfFormat::from_media_type(mime) == Some(self.format) ||
                matches!(
                    mime.split(';').next().unwrap_or(mime).trim(),
                    "text/plain" | "application/octet-stream"
                ),
            "RDF URL Content-Type disagrees with configured format"
        );
        ensure!(
            response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_none_or(|value| value == "identity"),
            "RDF URL uses unsupported content encoding"
        );
        let base = checked_url(response.url().as_str())?;
        // The synchronous anonymous-tempfile primitive is isolated from the
        // async collection task; all spool writes, seeking and parsing
        // are async.
        let spool = tokio::task::spawn_blocking(tempfile::tempfile).await??;
        let mut spool = BufWriter::new(tokio::fs::File::from_std(spool));
        let mut hash = Sha256::new();
        hash.update(format!(
            "{}\n{base}\n{:?}\n",
            self.url, self.format
        ));
        let mut size = 0u64;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            size += chunk.len() as u64;
            ensure!(
                size <= MAX_BYTES,
                "RDF URL exceeds 8 MiB byte limit"
            );
            hash.update(&chunk);
            spool.write_all(&chunk).await?;
        }
        spool.flush().await?;
        let mut spool = spool.into_inner();
        spool.seek(SeekFrom::Start(0)).await?;
        let revision = format!("rdf-url-v1:{}", hex::encode(hash.finalize()));
        let mut blanks = BlankNodes::new(
            self.project.as_str(),
            self.processor.as_str(),
            self.url.as_str(),
            &revision,
        );
        let mut modules = ModuleHints::default();
        let mut parser = RdfParser::from_format(self.format)
            .with_base_iri(base.as_str())?
            .for_tokio_async_reader(BufReader::new(spool));
        while let Some(quad) = parser.next().await {
            let quad = blanks.normalize(quad?);
            modules.observe(&quad)?;
            output.write(quad).await?;
        }
        for quad in modules.finish()? {
            output.write(quad).await?;
        }
        Ok(revision)
    }
}
