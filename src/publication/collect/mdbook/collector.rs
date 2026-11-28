//! The mdBook collector: `book.toml` gives the project, `SUMMARY.md` the
//! navigation. No preprocessor, renderer or script is ever run.
use {
    super::{
        config::{BookConfig, MAX_CONFIG_BYTES},
        summary::{MAX_SUMMARY_BYTES, Summary},
        warning::MdBookWarning,
    },
    crate::publication::{
        collect::{Collector, SourceIdentity},
        dataset::identity,
        term,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

const SUMMARY_FILE: &str = "SUMMARY.md";

/// One mdBook, identified by its `book.toml`.
pub(crate) struct MdBook {
    pub project: NamedNode,
    pub target:  NamedNode,
    root:        PathBuf,
    path:        PathBuf,
    source:      String,
}

impl MdBook {
    pub(crate) async fn new(root: &Path, path: &Path) -> Result<Self> {
        ensure!(
            tokio::fs::symlink_metadata(path).await?.is_file(),
            "book.toml must be a regular file"
        );
        let source = url::Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("invalid book.toml path"))?
            .to_string();
        let project = identity("mdbook-project", &[&source]);
        let target = identity("mdbook-target", &[project.as_str()]);
        Ok(Self {
            project,
            target,
            root: root.to_owned(),
            path: path.to_owned(),
            source,
        })
    }

    async fn read(path: &Path, limit: u64) -> Result<String> {
        ensure!(
            tokio::fs::metadata(path).await?.len() <= limit,
            "{} exceeds {limit} bytes",
            path.display()
        );
        let mut body = String::new(); // bounded: at most limit plus one byte.
        tokio::fs::File::open(path)
            .await?
            .take(limit + 1)
            .read_to_string(&mut body)
            .await?;
        ensure!(
            body.len() as u64 <= limit,
            "{} grew while read",
            path.display()
        );
        Ok(body)
    }

    async fn diagnose(
        &self,
        stream: &mut ContributionStream,
        source: &NamedNode,
        warning: &MdBookWarning,
    ) -> Result<()> {
        let message = warning.to_string();
        eprintln!("mdBook import: {message}");
        let diagnostic = identity("mdbook-import-diagnostic", &[
            &self.source,
            &message,
        ]);
        for quad in [
            q(source, "diagnostic", diagnostic.clone()),
            q(&diagnostic, "type", term("Diagnostic")),
            q(&diagnostic, "severity", lit("warning")),
            q(&diagnostic, "code", lit(warning.code())),
            q(&diagnostic, "message", lit(&message)),
            q(&diagnostic, "about", source.clone()),
        ] {
            stream.write(quad).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Collector for MdBook {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: "urn:grapharch:mdbook-configuration",
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let body = Self::read(&self.path, MAX_CONFIG_BYTES).await?;
        let config = BookConfig::parse(&body)?;
        let docs = crate::publication::website::discovery::directory(
            &self.root,
            &config.source,
        )
        .await?;
        ensure!(
            docs != self.root,
            "book.toml src must be a child directory of the book"
        );
        let outline =
            Self::read(&docs.join(SUMMARY_FILE), MAX_SUMMARY_BYTES).await?;
        let summary = Summary::parse(&outline)?;
        let profile = identity("mdbook-profile", &[self.project.as_str()]);
        let scope = identity("mdbook-scope", &[self.project.as_str()]);
        let source = NamedNode::new(&self.source)?;
        let renderer = crate::publication::Renderer::Website.iri();
        for quad in [
            q(&self.project, "type", term("Project")),
            title(&self.project, &config.title),
            q(
                &self.project,
                "contentSource",
                lit(&config.source),
            ),
            q(
                &self.project,
                "excludeSourcePath",
                lit(SUMMARY_FILE),
            ),
            q(&self.project, "target", self.target.clone()),
            q(&self.target, "type", term("Target")),
            q(&self.target, "scope", scope.clone()),
            q(&self.target, "profile", profile.clone()),
            q(&self.target, "renderer", renderer),
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
            dc(
                &self.project,
                "http://www.w3.org/ns/prov#wasDerivedFrom",
                source.clone(),
            ),
        ] {
            stream.write(quad).await?;
        }
        if let Some(description) = &config.description {
            stream
                .write(dc(
                    &self.project,
                    "http://purl.org/dc/terms/description",
                    lit(description),
                ))
                .await?;
        }
        if let Some(repository) = &config.repository {
            stream
                .write(q(
                    &self.project,
                    "repository",
                    NamedNode::new(repository.as_str())?,
                ))
                .await?;
        }
        if let Some(home) = summary.home(&docs).await? {
            stream
                .write(q(&self.project, "homeSourcePath", lit(&home)))
                .await?;
        }
        for warning in &config.warnings {
            self.diagnose(stream, &source, warning).await?;
        }
        summary.write(stream, &profile, &docs).await?;
        let mut digest = Sha256::new();
        digest.update(body.as_bytes());
        digest.update([0]);
        digest.update(outline.as_bytes());
        Ok(format!(
            "mdbook-v1:{}",
            hex::encode(digest.finalize())
        ))
    }
}

fn lit(value: &str) -> Literal { Literal::new_simple_literal(value) }

fn dc(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(predicate),
        value,
        GraphName::DefaultGraph,
    )
}

pub(super) fn q(
    subject: &NamedNode,
    predicate: &str,
    value: impl Into<Term>,
) -> Quad {
    let predicate = if predicate == "type" {
        NamedNode::new_unchecked(crate::publication::RDF_TYPE)
    } else {
        term(predicate)
    };
    Quad::new(
        subject.clone(),
        predicate,
        value,
        GraphName::DefaultGraph,
    )
}

pub(super) fn title(subject: &NamedNode, value: &str) -> Quad {
    dc(
        subject,
        "http://purl.org/dc/terms/title",
        lit(value),
    )
}
