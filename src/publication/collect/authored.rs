use {
    super::{Collector, source::FileInput},
    crate::publication::writer::ContributionStream,
    anyhow::{Context, Result, bail, ensure},
    async_trait::async_trait,
    oxrdf::NamedNode,
    serde::Deserialize,
    std::path::PathBuf,
};

/// A Markdown-file collector supporting frontmatter and plain Markdown bodies.
pub struct MarkdownFile(FileInput);
/// A SPARQL-file collector that documents statements without executing them.
pub struct SparqlFile(FileInput);

#[derive(Default, Deserialize)]
struct Frontmatter {
    title:     Option<String>,
    #[serde(default)]
    grapharch: Hints,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hints {
    subject: Option<String>,
}

impl MarkdownFile {
    /// Configure one collector instance and source file.
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self(FileInput::new(project, collector, path)?))
    }

    /// Stable identity of the authored fragment.
    pub fn fragment_id(&self) -> NamedNode { self.0.fragment_id() }
}

impl SparqlFile {
    /// Configure one statement source.
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self(FileInput::new(project, collector, path)?))
    }

    /// Stable identity of the statement fragment.
    pub fn fragment_id(&self) -> NamedNode { self.0.fragment_id() }
}

#[async_trait]
impl Collector for MarkdownFile {
    fn identity(&self) -> Result<super::SourceIdentity<'_>> {
        self.0.identity()
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let (source, revision) = self.0.authored().await?;
        let normalized = source.replace("\r\n", "\n");
        let (frontmatter, body) =
            if let Some(rest) = normalized.strip_prefix("---\n") {
                let Some(end) = rest.find("\n---\n") else {
                    bail!(
                        "{}: unterminated frontmatter",
                        self.0.path.display()
                    )
                };
                (
                    serde_yaml::from_str::<Frontmatter>(&rest[..end])
                        .context("invalid Markdown frontmatter")?,
                    &rest[end + 5..],
                )
            } else {
                (Frontmatter::default(), normalized.as_str())
            };
        let subject = frontmatter
            .grapharch
            .subject
            .map(NamedNode::new)
            .transpose()?;
        let quads = self.0.description(
            body,
            "text/markdown",
            subject,
            frontmatter.title.as_deref(),
            &revision,
        )?;
        stream.push(quads).await?;
        Ok(revision)
    }
}

#[async_trait]
impl Collector for SparqlFile {
    fn identity(&self) -> Result<super::SourceIdentity<'_>> {
        self.0.identity()
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let (source, revision) = self.0.authored().await?;
        let mut subject = None;
        for line in source.lines() {
            if let Some(pragma) = line.trim().strip_prefix("# grapharch:") {
                let Some(value) = pragma.strip_prefix("subject ") else {
                    bail!("unsupported SPARQL documentation pragma: {pragma}")
                };
                ensure!(
                    subject.is_none(),
                    "conflicting SPARQL subject pragmas"
                );
                subject = Some(NamedNode::new(value.trim())?);
            }
        }
        let quads = self.0.description(
            &source,
            "application/sparql-query",
            subject,
            None,
            &revision,
        )?;
        stream.push(quads).await?;
        Ok(revision)
    }
}
