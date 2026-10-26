//! Bounded transport for one blob at an already pinned repository revision.
use {
    super::*,
    crate::publication::collect::{
        MarkdownFile,
        source::{FileInput, MAX_AUTHORED_BYTES},
    },
};

#[derive(Clone)]
pub struct PinnedGitBlob {
    repository: PathBuf,
    relative:   String,
    blob:       gix::hash::ObjectId,
    revision:   Arc<Revision>,
    source:     String,
    location:   String,
}
impl RepositoryScan {
    /// Resolve a regular file within this exact fetched commit. Never
    /// re-fetches or reads a working tree, and never follows symlinks or
    /// submodules.
    pub async fn blob(&self, relative: &Path) -> Result<PinnedGitBlob> {
        ensure!(
            !self.revision.working_tree,
            "Git blob transport requires a remote pinned tree"
        );
        ensure!(
            !relative.as_os_str().is_empty() &&
                relative
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "Git blob path must be source-relative without traversal"
        );
        let path = self.root.clone();
        let commit = self.commit.clone();
        let relative = relative
            .to_str()
            .context("Git blob path must be UTF-8")?
            .to_owned();
        let requested = relative.clone();
        let blob = tokio::task::spawn_blocking(move || -> Result<_> {
            let repo = gix::open(path)?;
            let tree = repo
                .find_commit(gix::hash::ObjectId::from_hex(commit.as_bytes())?)?
                .tree()?;
            let entry =
                tree.lookup_entry_by_path(&requested)?.with_context(|| {
                    format!("missing required Git blob {requested}")
                })?;
            ensure!(
                matches!(
                    entry.mode().kind(),
                    gix::objs::tree::EntryKind::Blob |
                        gix::objs::tree::EntryKind::BlobExecutable
                ),
                "Git source must be a regular blob: {requested}"
            );
            Ok(entry.object_id())
        })
        .await??;
        Ok(PinnedGitBlob {
            repository: self.root.clone(),
            source: identity("git-file", &[&self.revision.origin, &relative])
                .into_string(),
            location: blob_url(&self.revision.origin, &self.commit, &relative)?,
            relative,
            blob,
            revision: self.revision.clone(),
        })
    }
}
impl PinnedGitBlob {
    pub fn commit(&self) -> &str { &self.revision.commit }

    pub fn source(&self) -> &str { &self.source }

    pub fn location(&self) -> &str { &self.location }

    pub fn revision(&self) -> String {
        format!(
            "git-blob-v1:{}:{}",
            self.revision.commit, self.blob
        )
    }

    pub async fn bytes(&self, limit: u64) -> Result<Vec<u8>> {
        ensure!(
            limit > 0 && limit <= MAX_RDF_BYTES,
            "invalid Git blob byte limit"
        );
        let (path, id) = (self.repository.clone(), self.blob);
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let repo = gix::open(path)?;
            ensure!(
                repo.find_header(id)?.size() <= limit,
                "Git blob exceeds configured byte limit"
            );
            let mut blob = repo.find_object(id)?.try_into_blob()?;
            ensure!(
                blob.data.len() as u64 <= limit,
                "Git blob exceeds configured byte limit"
            );
            Ok(std::mem::take(&mut blob.data))
        })
        .await?
    }

    pub fn rdf_collector(&self, processor: &str) -> Result<Arc<dyn Collector>> {
        NamedNode::new(processor)?;
        Ok(Arc::new(GitRdfFile {
            project:    self.revision.project.clone(),
            processor:  processor.into(),
            source:     self.source.clone(),
            base:       self.location.clone(),
            relative:   self.relative.clone(),
            repository: self.repository.clone(),
            blob:       self.blob,
            revision:   self.revision.as_ref().clone(),
        }))
    }

    async fn provenance(&self, stream: &mut ContributionStream) -> Result<()> {
        for (predicate, object) in [
            (
                "http://www.w3.org/ns/prov#wasDerivedFrom",
                self.revision.entity(),
            ),
            (
                "http://www.w3.org/ns/prov#atLocation",
                NamedNode::new(&self.location)?,
            ),
        ] {
            stream
                .write(Quad::new(
                    NamedNode::new(&self.source)?,
                    NamedNode::new(predicate)?,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(())
    }
}
/// One Markdown document; transport is independent of Markdown normalization.
pub struct GitMarkdownFile {
    blob:      PinnedGitBlob,
    processor: String,
    fragment:  NamedNode,
    website:   bool,
    hints:     Option<(String, String)>,
}
impl GitMarkdownFile {
    pub fn new(
        blob: PinnedGitBlob,
        processor: &str,
        fragment: NamedNode,
        website: bool,
    ) -> Result<Self> {
        NamedNode::new(processor)?;
        Ok(Self {
            blob,
            processor: processor.into(),
            fragment,
            website,
            hints: None,
        })
    }

    pub fn fragment_id(&self) -> NamedNode { self.fragment.clone() }

    pub fn with_page_hints(
        mut self,
        title: String,
        route: String,
    ) -> Result<Self> {
        ensure!(
            self.website && !title.trim().is_empty() && title.len() <= 1024,
            "invalid Git Markdown page hints"
        );
        super::super::page::route(&self.blob.relative, Some(&route))?;
        self.hints = Some((title, route));
        Ok(self)
    }
}
#[async_trait]
impl Collector for GitMarkdownFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.blob.revision.project,
            processor: &self.processor,
            source:    &self.blob.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let text =
            String::from_utf8(self.blob.bytes(MAX_AUTHORED_BYTES).await?)
                .context("Git Markdown must be UTF-8")?;
        let input = FileInput::supplied(
            &self.blob.revision.project,
            &self.processor,
            PathBuf::from(&self.blob.relative),
            self.blob.source.clone(),
            self.fragment.clone(),
            text,
            self.blob.revision(),
        )?;
        let revision =
            MarkdownFile::supplied(input, self.website, self.hints.clone())
                .produce(stream)
                .await?;
        self.blob.provenance(stream).await?;
        Ok(revision)
    }
}
