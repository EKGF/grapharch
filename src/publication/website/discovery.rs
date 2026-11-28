//! Bounded filesystem discovery with source-owned project exclusions.
use {
    super::exclusions::SourceExclusions,
    crate::publication::{
        collect::LocalImageFile,
        source_walk::SourceWalk,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

pub(crate) async fn content_root(
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

pub(crate) async fn directory(root: &Path, value: &str) -> Result<PathBuf> {
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

pub(crate) async fn exclusions(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<SourceExclusions> {
    SourceExclusions::read(root, view, project).await
}

pub(super) async fn discover(
    root: &Path,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<Vec<PathBuf>> {
    SourceWalk::new(root, website_source)
        .configured(view, project)
        .await?
        .files()
        .await
}

fn website_source(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "CNAME") ||
        path.extension().is_some_and(|ext| {
            matches!(ext.to_str(), Some("md" | "ttl" | "txt"))
        }) ||
        LocalImageFile::supported(path)
}
