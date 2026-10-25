//! Small source adapters that normalize authored content and native hints to
//! RDF.

mod authored;
mod blank_nodes;
pub mod git;
mod hashing_reader;
mod markdown_structure;
pub(crate) mod page;
pub(crate) mod published_artifact;
mod rdf;
mod source;
mod text_asset;
mod w3c;

use {
    super::writer::{ContributionStream, GraphWriter},
    anyhow::{Result, ensure},
    async_trait::async_trait,
    std::sync::Arc,
    tokio::task::JoinSet,
};
pub use {
    authored::{MarkdownFile, SparqlFile},
    rdf::RdfFile,
    text_asset::TextAssetFile,
    w3c::W3cSpecificationFile,
};

/// Stable ownership supplied by one specialized collector instance.
pub struct SourceIdentity<'a> {
    /// Project owning this contribution.
    pub project:   &'a str,
    /// Configured collector instance IRI.
    pub processor: &'a str,
    /// Source identity, independent of its current contents.
    pub source:    &'a str,
}

/// A specialized source adapter. Implementations never choose publication
/// placement.
#[async_trait]
pub trait Collector: Send + Sync {
    /// Identify the source before attempting I/O or native syntax parsing.
    fn identity(&self) -> Result<SourceIdentity<'_>>;
    /// Stream normalized assertions and return the observed
    /// source/configuration revision.
    async fn produce(&self, output: &mut ContributionStream) -> Result<String>;
    /// Register the attempt first, so even early read/parse failures block
    /// assembly.
    async fn collect(&self, writer: &GraphWriter) -> Result<()> {
        let identity = self.identity()?;
        let mut stream = writer
            .begin_pending(
                identity.project,
                identity.processor,
                identity.source,
            )
            .await?;
        let revision = match self.produce(&mut stream).await {
            Ok(revision) => revision,
            Err(error) => {
                stream.abort().await?;
                let diagnostic = super::diagnostics::Diagnostic {
                    severity:        super::diagnostics::Severity::Error,
                    code:            "collection-failed".into(),
                    message:         format!("{error:#}"),
                    reference:       identity.source.into(),
                    source_fragment: None,
                };
                if let Err(record_error) = writer
                    .record_diagnostic(
                        identity.project,
                        identity.processor,
                        identity.source,
                        "collect",
                        diagnostic,
                    )
                    .await
                {
                    return Err(error.context(format!(
                        "diagnostic recording also failed: {record_error:#}"
                    )));
                }
                return Err(error);
            },
        };
        if let Err(error) = stream.set_revision(&revision).await {
            stream.abort().await?;
            return Err(error);
        }
        stream.complete().await
    }
}

/// Completed independent work and retained errors from a collector batch.
pub struct CollectionReport {
    /// Successfully completed inputs.
    pub completed: usize,
    /// Errors that prevent a clean project build.
    pub errors:    Vec<super::diagnostics::Diagnostic>,
}

/// Schedule a lazy collector iterator with bounded concurrency, failing fast.
pub async fn collect_all(
    work: impl IntoIterator<Item = Arc<dyn Collector>>,
    writer: &GraphWriter,
    concurrency: usize,
) -> Result<()> {
    collect_all_with_mode(
        work,
        writer,
        concurrency,
        super::diagnostics::FailureMode::FailFast,
    )
    .await?;
    Ok(())
}

/// Continue independent inputs only when explicitly requested. Failed inputs
/// still block assembly; errors are recorded as KG documentation fragments.
pub async fn collect_all_with_mode(
    work: impl IntoIterator<Item = Arc<dyn Collector>>,
    writer: &GraphWriter,
    concurrency: usize,
    mode: super::diagnostics::FailureMode,
) -> Result<CollectionReport> {
    ensure!(
        concurrency > 0,
        "collector concurrency must be positive"
    );
    let mut tasks = JoinSet::new();
    let mut report = CollectionReport { completed: 0, errors: Vec::new() };
    for collector in work {
        if tasks.len() == concurrency {
            outcome(
                tasks.join_next().await.ok_or_else(|| {
                    anyhow::anyhow!("collector task missing")
                })??,
                mode,
                &mut report,
            )?;
        }
        let writer = writer.clone();
        tasks.spawn(async move { collector.collect(&writer).await });
    }
    while let Some(result) = tasks.join_next().await {
        outcome(result?, mode, &mut report)?;
    }
    Ok(report)
}

fn outcome(
    result: Result<()>,
    mode: super::diagnostics::FailureMode,
    report: &mut CollectionReport,
) -> Result<()> {
    match result {
        Ok(()) => report.completed += 1,
        Err(error) if mode == super::diagnostics::FailureMode::FailFast => {
            return Err(error)
        },
        Err(error) => {
            ensure!(
                report.errors.len() < 1024,
                "collector diagnostic budget exceeded"
            );
            report.errors.push(super::diagnostics::Diagnostic {
                severity:        super::diagnostics::Severity::Error,
                code:            "collection-failed".into(),
                message:         format!("{error:#}"),
                reference:       String::new(),
                source_fragment: None,
            });
        },
    }
    Ok(())
}
