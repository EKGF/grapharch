//! Bounded, non-executing import of source-owned MkDocs instructions.
mod config;
mod extensions;
pub(super) mod metadata;
mod navigation;
mod paths;
use {
    super::{Collector, SourceIdentity},
    crate::publication::{dataset::identity, term, writer::ContributionStream},
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

pub(crate) struct MkDocs {
    pub project: NamedNode,
    pub target:  NamedNode,
    root:        PathBuf,
    path:        PathBuf,
    source:      String,
}
impl MkDocs {
    /// Bind an immutable transport's logical source instead of its temporary
    /// path.
    pub(crate) fn with_source_identity(mut self, source: &str) -> Result<Self> {
        self.project = Self::project_for_source(source)?;
        self.source = source.to_owned();
        self.target = identity("mkdocs-target", &[self.project.as_str()]);
        Ok(self)
    }

    pub(crate) fn project_for_source(source: &str) -> Result<NamedNode> {
        NamedNode::new(source)?;
        Ok(identity("mkdocs-project", &[source]))
    }

    pub async fn new(root: &Path, path: &Path) -> Result<Self> {
        ensure!(
            tokio::fs::symlink_metadata(path).await?.is_file(),
            "MkDocs configuration must be a regular file"
        );
        let source = url::Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("invalid MkDocs source path"))?
            .to_string();
        let project = identity("mkdocs-project", &[&source]);
        let target = identity("mkdocs-target", &[project.as_str()]);
        Self {
            project,
            target,
            root: root.to_owned(),
            path: path.to_owned(),
            source: source.clone(),
        }
        .with_source_identity(&source)
    }
}
#[async_trait]
impl Collector for MkDocs {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: "urn:grapharch:mkdocs-configuration",
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        ensure!(
            tokio::fs::metadata(&self.path).await?.len() <= 128 * 1024,
            "MkDocs configuration exceeds 128 KiB"
        );
        let mut body = String::new(); // bounded: read at most128KiB plus one byte to detect overflow.
        tokio::fs::File::open(&self.path)
            .await?
            .take(128 * 1024 + 1)
            .read_to_string(&mut body)
            .await?;
        let config = config::Config::parse(&body)?;
        let docs = crate::publication::website::discovery::directory(
            &self.root,
            &config.docs,
        )
        .await?;
        ensure!(
            docs != self.root,
            "MkDocs docs_dir must be a child directory of the project"
        );
        let profile = identity("mkdocs-profile", &[self.project.as_str()]);
        let scope = identity("mkdocs-scope", &[self.project.as_str()]);
        let source = NamedNode::new(&self.source)?;
        for quad in [
            q(&self.project, "type", term("Project")),
            title(&self.project, &config.title),
            q(&self.project, "contentSource", lit(&config.docs)),
            q(&self.project, "target", self.target.clone()),
            q(&self.target, "type", term("Target")),
            q(&self.target, "scope", scope.clone()),
            q(&self.target, "profile", profile.clone()),
            q(&self.target, "renderer", lit("website")),
            q(&self.target, "destination", lit("site")),
            q(&scope, "type", term("Scope")),
            q(&profile, "type", term("Profile")),
            q(&profile, "layout", lit("documentation")),
            q(&source, "sourceMarkup", lit(&body)),
            q(
                &source,
                "contentDigest",
                lit(&hex::encode(Sha256::digest(body.as_bytes()))),
            ),
            Quad::new(
                self.project.clone(),
                NamedNode::new_unchecked(
                    "http://www.w3.org/ns/prov#wasDerivedFrom",
                ),
                source.clone(),
                GraphName::DefaultGraph,
            ),
        ] {
            stream.write(quad).await?;
        }
        for root in &config.hints.roots {
            crate::publication::website::discovery::directory(&self.root, root)
                .await?;
            stream
                .write(q(&self.project, "includeRoot", lit(root)))
                .await?;
        }
        for requested in &config.hints.dictionaries {
            let path = paths::appended_source(
                &self.root,
                &config.hints.roots,
                requested,
            )
            .await?;
            let source = self.root.join(&path);
            if source.starts_with(&docs) {
                stream
                    .write(q(
                        &self.project,
                        "excludeSourcePath",
                        lit(&super::page::source_path(&docs, &source)?),
                    ))
                    .await?;
            }
            let collector = identity("mkdocs-abbreviations", &[
                self.project.as_str(),
                &path,
            ]);
            for quad in [
                q(&self.project, "collector", collector.clone()),
                q(&collector, "type", term("Collector")),
                q(&collector, "kind", lit("abbreviation-file")),
                q(&collector, "source", lit(&path)),
            ] {
                stream.write(quad).await?;
            }
        }
        if let Some(value) = config.site_url {
            let mut url = url::Url::parse(&value)?;
            ensure!(
                matches!(url.scheme(), "http" | "https") &&
                    url.host_str().is_some() &&
                    url.username().is_empty() &&
                    url.password().is_none() &&
                    url.query().is_none() &&
                    url.fragment().is_none(),
                "site_url must be an absolute HTTP(S) directory URL without \
                 credentials/query/fragment"
            );
            if !url.path().ends_with('/') {
                url.set_path(&format!("{}/", url.path()));
            }
            stream
                .write(q(
                    &self.target,
                    "thisVersion",
                    NamedNode::new(url.as_str())?,
                ))
                .await?;
        }
        if let Some(value) = config.description {
            stream
                .write(Quad::new(
                    self.project.clone(),
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/description",
                    ),
                    lit(&value),
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        if let Some(value) = config.repository {
            let url = url::Url::parse(&value)?;
            ensure!(
                matches!(url.scheme(), "http" | "https"),
                "repo_url must use HTTP(S)"
            );
            stream
                .write(q(
                    &self.project,
                    "repository",
                    NamedNode::new(value)?,
                ))
                .await?;
        }
        for warning in config.warnings {
            eprintln!("MkDocs import: {warning}");
            let diagnostic = identity("mkdocs-import-diagnostic", &[
                &self.source,
                &warning,
                &body,
            ]);
            for quad in [
                q(&source, "diagnostic", diagnostic.clone()),
                q(&diagnostic, "type", term("Diagnostic")),
                q(&diagnostic, "severity", lit("warning")),
                q(
                    &diagnostic,
                    "code",
                    lit("mkdocs-presentation-substitution"),
                ),
                q(&diagnostic, "message", lit(&warning)),
                q(&diagnostic, "about", source.clone()),
            ] {
                stream.write(quad).await?;
            }
        }
        if let Some(nav) = config.nav {
            navigation::write(stream, &profile, &docs, &nav).await?;
        } else {
            let collector = identity("mkdocs-directory-navigation", &[self
                .project
                .as_str()]);
            for quad in [
                q(&self.project, "collector", collector.clone()),
                q(&collector, "type", term("Collector")),
                q(
                    &collector,
                    "kind",
                    lit("documentation-navigation"),
                ),
                q(&collector, "source", lit(&config.docs)),
                q(&collector, "navigationProfile", profile),
            ] {
                stream.write(quad).await?;
            }
        }
        Ok(format!(
            "mkdocs-v1:{}",
            hex::encode(Sha256::digest(body.as_bytes()))
        ))
    }
}
fn lit(value: &str) -> Literal { Literal::new_simple_literal(value) }
fn q(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        if predicate == "type" {
            NamedNode::new_unchecked(crate::publication::RDF_TYPE)
        } else {
            term(predicate)
        },
        value,
        GraphName::DefaultGraph,
    )
}
fn title(subject: &NamedNode, value: &str) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        lit(value),
        GraphName::DefaultGraph,
    )
}

pub(crate) fn validate_yaml_syntax(body: &str) -> Result<()> {
    config::reject_tags(body)
}
