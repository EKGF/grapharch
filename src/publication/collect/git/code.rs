//! One retained UTF-8 code example from a pinned Git tree.
use {
    super::PinnedGitBlob,
    crate::publication::{
        collect::{
            Collector,
            SourceIdentity,
            source::{FileInput, MAX_AUTHORED_BYTES},
        },
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::NamedNode,
    std::path::PathBuf,
};

/// Collect one code file without executing it or fetching its dependencies.
pub struct GitCodeFile {
    project:   String,
    processor: String,
    blob:      PinnedGitBlob,
    fragment:  NamedNode,
    subject:   NamedNode,
    format:    String,
}

impl GitCodeFile {
    /// Bind an explicitly typed source example to its documentable subject.
    pub fn new(
        project: &str,
        processor: &str,
        blob: PinnedGitBlob,
        fragment: NamedNode,
        subject: NamedNode,
        format: &str,
    ) -> Result<Self> {
        NamedNode::new(project)?;
        NamedNode::new(processor)?;
        ensure!(
            matches!(
                format,
                "application/ld+json" |
                    "application/json" |
                    "text/turtle" |
                    "application/sparql-query" |
                    "text/plain"
            ),
            "unsupported code fragment format {format}"
        );
        Ok(Self {
            project: project.into(),
            processor: processor.into(),
            blob,
            fragment,
            subject,
            format: format.into(),
        })
    }
}

#[async_trait]
impl Collector for GitCodeFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    self.blob.source(),
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let body =
            String::from_utf8(self.blob.bytes(MAX_AUTHORED_BYTES).await?)?;
        ensure!(
            !body.trim().is_empty(),
            "code example cannot be empty"
        );
        let revision = format!(
            "git-code-v1:{}:{}:{}:{}",
            self.blob.revision(),
            self.fragment,
            self.subject,
            self.format
        );
        let input = FileInput::supplied(
            &self.project,
            &self.processor,
            PathBuf::from(self.blob.location()),
            self.blob.source().into(),
            self.fragment.clone(),
            body.clone(),
            revision.clone(),
        )?;
        stream
            .push(input.description(
                &body,
                &self.format,
                Some(self.subject.clone()),
                None,
                &revision,
            )?)
            .await?;
        Ok(revision)
    }
}
