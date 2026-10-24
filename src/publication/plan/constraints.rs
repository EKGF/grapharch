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
