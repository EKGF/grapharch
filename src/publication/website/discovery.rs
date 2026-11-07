//! Bounded filesystem discovery with source-owned project exclusions.
use {
    crate::publication::{collect::LocalImageFile, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

async fn exclusions(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new(); // bounded: 64 authored exclusions.
    let mut rows = view.objects(project, "excludeSourcePath").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            paths.len() < 64,
            "source exclusions exceed 64 paths"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("source exclusion must be a path literal")
        };
        let path = Path::new(value.value());
        ensure!(
            !value.value().is_empty() &&
                path.components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "source exclusion must be a relative path without traversal"
        );
        let path = root.join(path);
        let metadata = tokio::fs::symlink_metadata(&path).await?;
        ensure!(
            !metadata.is_symlink(),
            "source exclusion must not be a symlink"
        );
        ensure!(
            tokio::fs::canonicalize(&path).await?.starts_with(root),
            "source exclusion escapes documentation root"
        );
        paths.push(path);
    }
    Ok(paths)
}

pub(super) async fn discover(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<Vec<PathBuf>> {
    let exclusions = exclusions(root, view, project).await?;
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            visited += 1;
            ensure!(
                visited <= 4096,
                "website discovery exceeds 4096-entry budget"
            );
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                anyhow::anyhow!("website filenames must be UTF-8")
            })?;
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            if exclusions
                .iter()
                .any(|excluded| entry.path().starts_with(excluded))
            {
                continue;
            }
            let kind = entry.file_type().await?;
            ensure!(
                !kind.is_symlink(),
                "website discovery rejects symlinks: {}",
                entry.path().display()
            );
            if kind.is_dir() {
                pending.push(entry.path());
            } else if name == "CNAME" ||
                entry.path().extension().is_some_and(|ext| {
                    matches!(ext.to_str(), Some("md" | "ttl" | "txt")) ||
                        LocalImageFile::supported(&entry.path())
                })
            {
                files.push(entry.path());
                ensure!(
                    files.len() <= 256,
                    "website exceeds 256-source budget"
                );
            }
        }
    }
    files.sort();
    Ok(files)
}
