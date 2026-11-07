//! Bounded filesystem discovery with source-owned project exclusions.
use {
    crate::publication::{collect::LocalImageFile, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

pub(super) async fn content_root(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<PathBuf> {
    let Some(value) = view.optional(project, "contentSource").await? else {
        return Ok(root.to_owned());
    };
    let Term::Literal(value) = value else {
        anyhow::bail!("contentSource must be a relative directory path literal")
    };
    directory(root, value.value()).await
}

pub(super) async fn include_roots(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new(); // bounded: eight explicitly configured include roots.
    let mut values = view.objects(project, "includeRoot").await?;
    while let Some(row) = values.next().await? {
        ensure!(
            roots.len() < 8,
            "includeRoot exceeds eight-directory budget"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!(
                "includeRoot must be a relative directory path literal"
            )
        };
        roots.push(directory(root, value.value()).await?);
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

pub(super) async fn directory(root: &Path, value: &str) -> Result<PathBuf> {
    let relative = Path::new(value);
    ensure!(
        !value.is_empty() &&
            value.len() <= 2048 &&
            relative.components().all(|part| {
                matches!(part, Component::Normal(_) | Component::CurDir)
            }),
        "source directory must be relative without traversal"
    );
    let directory = tokio::fs::canonicalize(root.join(relative)).await?;
    ensure!(
        directory.starts_with(root) &&
            tokio::fs::metadata(&directory).await?.is_dir(),
        "source directory must be inside project root"
    );
    Ok(directory)
}

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
        let mut ancestor = root.to_owned();
        for part in path.components() {
            ancestor.push(part);
            match tokio::fs::symlink_metadata(&ancestor).await {
                Ok(metadata) => {
                    ensure!(
                        !metadata.is_symlink(),
                        "source exclusion must not traverse a symlink"
                    )
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    break
                },
                Err(error) => return Err(error.into()),
            }
        }
        // Excluded build output need not exist in a clean checkout.
        paths.push(root.join(path));
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
