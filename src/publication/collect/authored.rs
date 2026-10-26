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
pub struct MarkdownFile(
    FileInput,
    Option<PathBuf>,
    Option<(String, String)>,
    // Git-supplied Markdown provides its own repository-aware references.
    bool,
);
/// A SPARQL-file collector that documents statements without executing them.
pub struct SparqlFile(FileInput);

#[derive(Default, Deserialize)]
struct Frontmatter {
    title:     Option<String>,
    #[serde(default)]
    grapharch: Hints,
    #[serde(flatten)]
    page:      super::page::PageHints,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hints {
    subject:          Option<String>,
    #[serde(default)]
    showcase_gallery: bool,
}

impl MarkdownFile {
    pub(super) fn supplied(
        input: FileInput,
        website: bool,
        hints: Option<(String, String)>,
    ) -> Self {
        Self(input, website.then(PathBuf::new), hints, false)
    }

    /// Configure one collector instance and source file.
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self(
            FileInput::new(project, collector, path)?,
            None,
            None,
            true,
        ))
    }

    /// Collect page navigation and route hints relative to a documentation
    /// root.
    pub fn in_site(
        project: &str,
        collector: &str,
        root: PathBuf,
        path: PathBuf,
    ) -> Result<Self> {
        super::page::source_path(&root, &path)?;
        Ok(Self(
            FileInput::new(project, collector, path)?,
            Some(root),
            None,
            true,
        ))
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
        let (source, mut revision) = self.0.authored().await?;
        let normalized = source.replace("\r\n", "\n");
        let (mut frontmatter, body) =
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
        if let Some((title, route)) = &self.2 {
            ensure!(
                frontmatter
                    .title
                    .as_ref()
                    .is_none_or(|value| value == title),
                "conflicting Markdown title hints"
            );
            if let Some(value) = &frontmatter.page.permalink {
                ensure!(
                    super::page::route(self.0.source()?, Some(value))? ==
                        super::page::route(self.0.source()?, Some(route))?,
                    "conflicting Markdown route hints"
                );
            }
            frontmatter.title = Some(title.clone());
            frontmatter.page.permalink = Some(route.clone());
        }
        let mut subject = frontmatter
            .grapharch
            .subject
            .map(NamedNode::new)
            .transpose()?;
        let mut metadata = Vec::new();
        if let Some(root) = &self.1 {
            use oxrdf::Literal;
            let fragment =
                subject.clone().unwrap_or_else(|| self.fragment_id());
            let source = super::page::source_path(root, &self.0.path)?;
            revision = format!("site-page-v7:{source}:{revision}");
            subject = Some(fragment.clone());
            metadata.extend(super::markdown_structure::describe(
                &self.0, &fragment, body,
            )?);
            metadata.push(self.0.quad(
                &self.fragment_id(),
                "partOf",
                fragment.clone(),
            ));
            if let Some(title) = &frontmatter.title {
                metadata.push(oxrdf::Quad::new(
                    fragment.clone(),
                    NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                    Literal::new_simple_literal(title),
                    oxrdf::GraphName::DefaultGraph,
                ));
            }
            if frontmatter.grapharch.showcase_gallery {
                metadata.push(self.0.quad(
                    &fragment,
                    "showcaseGallery",
                    Literal::from(true),
                ));
            }
            metadata.extend([
                self.0.quad(
                    &fragment,
                    "type",
                    crate::publication::term("Page"),
                ),
                self.0.quad(
                    &fragment,
                    "sourcePath",
                    Literal::new_simple_literal(&source),
                ),
                self.0.quad(
                    &fragment,
                    "route",
                    Literal::new_simple_literal(super::page::route(
                        &source,
                        frontmatter.page.permalink.as_deref(),
                    )?),
                ),
                self.0.quad(
                    &fragment,
                    "navigationOrder",
                    Literal::from(frontmatter.page.nav_order.unwrap_or(999)),
                ),
                self.0.quad(
                    &fragment,
                    "navigationHidden",
                    Literal::from(frontmatter.page.nav_exclude),
                ),
            ]);
            if let Some(parent) = &frontmatter.page.parent {
                metadata.push(self.0.quad(
                    &fragment,
                    "parentTitle",
                    Literal::new_simple_literal(parent),
                ));
            }
            ensure!(
                frontmatter.title.is_some(),
                "{}: website pages require a frontmatter title",
                source
            );
        }
        if self.3 {
            let images = super::markdown_images::describe(&self.0, body)?;
            if !images.is_empty() {
                revision = format!("markdown-http-images-v1:{revision}");
            }
            metadata.extend(images);
        }
        let mut quads = self.0.description(
            body,
            "text/markdown",
            subject,
            frontmatter.title.as_deref(),
            &revision,
        )?;
        quads.extend(metadata);
        for quad in quads {
            stream.write(quad).await?;
        }
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
