//! Explicit documentation transport reuses the locked bare cache and commit
//! pin.
use super::*;

/// Stable source identity shared by remote configuration and Git blob
/// collectors.
pub async fn documentation_source(
    repository: &str,
    relative: &Path,
) -> Result<String> {
    validate_path(relative)?;
    let repository = repository.to_owned();
    let origin = tokio::task::spawn_blocking(move || {
        public_repository_url(&repository).map(|url| url.to_string())
    })
    .await??;
    Ok(identity("git-file", &[
        &origin,
        relative.to_str().context("Git path must be UTF-8")?,
    ])
    .into_string())
}

/// Pin a documentation repository without pretending it is an RDF repository.
/// Selection and materialization remain explicit and never execute source code.
pub async fn remote_documentation(
    url: &str,
    reference: &str,
    cache: &Path,
    project: &str,
) -> Result<RepositoryScan> {
    ensure!(
        !reference.is_empty(),
        "a Git ref or commit is required"
    );
    let (url, reference, cache, project) = (
        url.to_owned(),
        reference.to_owned(),
        cache.to_owned(),
        project.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        Ok(scan_remote_selected(
            &url,
            &reference,
            &cache,
            Some(&project),
            None,
            false,
        )?
        .0)
    })
    .await?
}

fn validate_path(path: &Path) -> Result<()> {
    ensure!(
        path.as_os_str().len() <= 4096,
        "documentation exceeds 4096-byte path budget"
    );
    ensure!(
        !path.as_os_str().is_empty() &&
            path.components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
        "documentation selection must be repository-relative without traversal"
    );
    Ok(())
}

impl RepositoryScan {
    /// Materialize only explicitly selected paths from this pinned tree for
    /// source adapters that need relative-file resolution. Never checks out Git
    /// hooks, follows links, executes files, or changes the bare cache.
    pub async fn materialize_documentation(
        &self,
        selected: &[PathBuf],
        destination: &Path,
    ) -> Result<Vec<PinnedGitBlob>> {
        ensure!(
            !selected.is_empty() && selected.len() <= 32,
            "documentation selection requires 1–32 paths"
        );
        for path in selected {
            validate_path(path)?;
        }
        ensure!(
            tokio::fs::symlink_metadata(destination).await?.is_dir(),
            "documentation destination must be a regular directory"
        );
        ensure!(
            tokio::fs::read_dir(destination)
                .await?
                .next_entry()
                .await?
                .is_none(),
            "documentation destination must be empty"
        );
        let (repository, commit, selected) = (
            self.root.clone(),
            self.commit.clone(),
            selected.to_vec(),
        );
        let paths = tokio::task::spawn_blocking(move || {
            selected_paths(&repository, &commit, &selected)
        })
        .await??;
        let mut blobs = Vec::new(); // bounded: at most 4096 explicitly selected regular files.
        let mut bytes = 0u64;
        for path in paths {
            let blob = self.blob(Path::new(&path)).await?;
            let body = blob.bytes(MAX_RDF_BYTES).await?;
            bytes += body.len() as u64;
            ensure!(
                bytes <= 128 * 1024 * 1024,
                "documentation materialization exceeds 128 MiB"
            );
            let target = destination.join(path);
            tokio::fs::create_dir_all(
                target.parent().context("document has no parent")?,
            )
            .await?;
            tokio::fs::write(target, body).await?;
            blobs.push(blob);
        }
        Ok(blobs)
    }
}

fn selected_paths(
    repository: &Path,
    commit: &str,
    selected: &[PathBuf],
) -> Result<Vec<String>> {
    let repo = gix::open(repository)?;
    let tree = repo
        .find_commit(gix::hash::ObjectId::from_hex(commit.as_bytes())?)?
        .tree()?;
    let mut pending = Vec::new(); // bounded: selected paths and at most 4096 pending directories.
    for path in selected {
        let entry = tree.lookup_entry_by_path(path)?.with_context(|| {
            format!(
                "missing selected documentation path {}",
                path.display()
            )
        })?;
        pending.push((
            path.to_str()
                .context("documentation path must be UTF-8")?
                .to_owned(),
            entry.object_id(),
            entry.mode().kind(),
        ));
    }
    let mut files = std::collections::BTreeSet::new(); // bounded: at most 4096 paths of at most 4096 bytes each.
    let mut count = 0usize;
    while let Some((path, id, kind)) = pending.pop() {
        count += 1;
        ensure!(
            count <= 100_000 && path.len() <= 4096,
            "documentation tree exceeds traversal budget"
        );
        match kind {
            gix::objs::tree::EntryKind::Tree => {
                ensure!(
                    repo.find_header(id)?.size() <= MAX_RDF_BYTES,
                    "documentation directory tree exceeds 8 MiB"
                );
                for entry in repo.find_object(id)?.try_into_tree()?.iter() {
                    let entry = entry?;
                    ensure!(
                        pending.len() < 4096,
                        "documentation exceeds 4096 pending entries"
                    );
                    let name = std::str::from_utf8(entry.filename())?;
                    ensure!(
                        path.len() + 1 + name.len() <= 4096,
                        "documentation exceeds 4096-byte path budget"
                    );
                    pending.push((
                        format!("{path}/{name}"),
                        entry.object_id(),
                        entry.mode().kind(),
                    ));
                }
            },
            gix::objs::tree::EntryKind::Blob |
            gix::objs::tree::EntryKind::BlobExecutable => {
                validate_path(Path::new(&path))?;
                ensure!(
                    files.len() < 4096 || files.contains(&path),
                    "documentation exceeds 4096 files"
                );
                files.insert(path);
            },
            _ => {
                anyhow::bail!(
                    "documentation source must not be a symlink or submodule: \
                     {path}"
                )
            },
        }
    }
    ensure!(
        !files.is_empty(),
        "documentation selection contains no files"
    );
    Ok(files.into_iter().collect())
}
