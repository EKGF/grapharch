use {
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

pub(super) fn integer(value: &Term, label: &str) -> Result<u32> {
    crate::publication::validation::unsigned(value, label)
}

pub(super) async fn destination_path(
    view: &RevisionView,
    target: &NamedNode,
) -> Result<PathBuf> {
    destination(&view.text(target, "destination").await?)
}

pub(super) fn destination(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty() && path.is_relative(),
        "target destination must be a nonempty relative artifact directory"
    );
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "target destination must not contain traversal or dot components"
    );
    Ok(path.to_owned())
}

/// Whether `container` declares `ga:containsTarget nested`, which lets the
/// nested target publish beneath the container's destination.
pub(super) async fn contains(
    view: &RevisionView,
    container: &NamedNode,
    nested: &NamedNode,
) -> Result<bool> {
    let mut rows = view.objects(container, "containsTarget").await?;
    while let Some(row) = rows.next().await? {
        if row["value"] == oxrdf::Term::NamedNode(nested.clone()) {
            return Ok(true);
        }
    }
    Ok(false)
}
