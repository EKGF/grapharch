//! Publication definitions pinned at one exact controlling Git revision. A
//! designated content branch never supplies GraphArch definitions.
use {
    super::*,
    gix::bstr::ByteSlice,
    sha2::{Digest, Sha256},
};

/// Provenance processor of the controlling definitions revision. It differs
/// from content provenance, so one origin observed at two commits keeps both
/// contributions.
pub const DEFINITIONS_PROVENANCE: &str =
    "urn:grapharch:git-provenance:definitions";

/// One committed definitions file, read from a Git object database, with the
/// provenance of its controlling revision.
pub struct CommittedDefinitions {
    blob:       PinnedGitBlob,
    provenance: Arc<dyn Collector>,
    sha256:     String,
}

impl CommittedDefinitions {
    /// Verify a controlling checkout strictly: its HEAD is exactly `commit`,
    /// the file is tracked and committed there, and the working file is
    /// byte-identical to the committed blob. The definitions are then read
    /// from the object database, never from the working tree.
    pub async fn checkout(
        path: &Path,
        commit: &str,
        project: &str,
    ) -> Result<Self> {
        NamedNode::new(project)?;
        let metadata = tokio::fs::symlink_metadata(path)
            .await
            .with_context(|| format!("read definitions {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            "definitions must be a regular file, not a link or directory: {}",
            path.display()
        );
        ensure!(
            metadata.len() <= MAX_RDF_BYTES,
            "definitions exceed the RDF byte budget"
        );
        let file = tokio::fs::canonicalize(path).await?;
        let bytes = tokio::fs::read(&file).await?;
        ensure!(
            bytes.len() as u64 <= MAX_RDF_BYTES,
            "definitions exceed the RDF byte budget"
        );
        let (commit, project) = (commit.to_owned(), project.to_owned());
        let (root, revision, relative) =
            tokio::task::spawn_blocking(move || {
                verify(&file, &bytes, &commit, &project)
            })
            .await??;
        Self::pinned(root, revision, Path::new(&relative)).await
    }

    /// Select a definitions file from an exact controlling commit already
    /// fetched into the bare transport cache.
    pub async fn fetched(scan: &RepositoryScan, path: &Path) -> Result<Self> {
        let mut revision = scan.revision.as_ref().clone();
        revision.processor = DEFINITIONS_PROVENANCE;
        Self::pinned(scan.root.clone(), revision, path).await
    }

    async fn pinned(
        root: PathBuf,
        revision: Revision,
        relative: &Path,
    ) -> Result<Self> {
        let revision = Arc::new(revision);
        let blob = PinnedGitBlob::at(root, revision.clone(), relative).await?;
        let sha256 =
            hex::encode(Sha256::digest(blob.bytes(MAX_RDF_BYTES).await?));
        Ok(Self { blob, provenance: revision, sha256 })
    }

    /// The pinned definitions blob.
    pub fn blob(&self) -> &PinnedGitBlob { &self.blob }

    /// Provenance of the controlling revision, under its own processor.
    pub fn provenance(&self) -> Arc<dyn Collector> { self.provenance.clone() }

    /// Public origin of the controlling repository.
    pub fn repository(&self) -> &str { self.blob.origin() }

    /// Repository-relative definitions path.
    pub fn path(&self) -> &str { self.blob.path() }

    /// Exact controlling commit.
    pub fn commit(&self) -> &str { self.blob.commit() }

    /// Git blob identity of the committed definitions.
    pub fn blob_id(&self) -> String { self.blob.blob_id() }

    /// SHA-256 digest of the committed definitions bytes.
    pub fn sha256(&self) -> &str { &self.sha256 }
}

fn verify(
    file: &Path,
    bytes: &[u8],
    commit: &str,
    project: &str,
) -> Result<(PathBuf, Revision, String)> {
    let parent = file.parent().context("definitions file has no parent")?;
    let repo = gix::discover(parent)
        .context("definitions must be inside a controlling Git checkout")?;
    let root = repo
        .workdir()
        .context("controlling checkout must have a working tree")?
        .canonicalize()?;
    let relative = file
        .strip_prefix(&root)
        .context("definitions must be inside the controlling checkout")?;
    let mut parts = Vec::new(); // bounded: components of one checked path.
    for part in relative.components() {
        let std::path::Component::Normal(part) = part else {
            anyhow::bail!("definitions path must be checkout-relative")
        };
        parts.push(part.to_str().context("definitions path must be UTF-8")?);
    }
    let name = parts.join("/");
    let expected = gix::hash::ObjectId::from_hex(commit.as_bytes())
        .context("definitions commit must be a hexadecimal Git object id")?;
    let head = repo
        .head_commit()
        .context("controlling checkout has no HEAD commit")?;
    ensure!(
        head.id == expected,
        "controlling checkout HEAD {} differs from definitions commit {commit}",
        head.id
    );
    let committed =
        head.tree()?.lookup_entry_by_path(&name)?.with_context(|| {
            format!("definitions {name} are not committed at {commit}")
        })?;
    ensure!(
        matches!(
            committed.mode().kind(),
            gix::objs::tree::EntryKind::Blob |
                gix::objs::tree::EntryKind::BlobExecutable
        ),
        "committed definitions must be a regular blob: {name}"
    );
    let index = repo.index()?;
    let tracked = index
        .entry_by_path(name.as_bytes().as_bstr())
        .with_context(|| format!("definitions {name} are not tracked"))?;
    let working = gix::objs::compute_hash(
        repo.object_hash(),
        gix::objs::Kind::Blob,
        bytes,
    )?;
    ensure!(
        tracked.id == committed.object_id() && working == committed.object_id(),
        "definitions differ from committed controlling revision {commit}: \
         {name}"
    );
    let mut revision = Revision::read(
        project,
        origin_url(&repo, &root)?,
        &repo,
        &head,
        false,
    )?;
    revision.processor = DEFINITIONS_PROVENANCE;
    Ok((root, revision, name))
}

#[cfg(test)]
mod tests;
