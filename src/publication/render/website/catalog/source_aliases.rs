//! Retained authored fragments can be composed into a differently routed page.
use {
    super::Catalog,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
pub(super) fn retain(
    catalog: &mut Catalog,
    fragment: &NamedNode,
    owner: &NamedNode,
    paths: Vec<Term>,
) -> Result<()> {
    if fragment == owner {
        return Ok(());
    }
    for value in paths {
        let Term::Literal(path) = value else {
            anyhow::bail!("fragment source path must be literal")
        };
        let path = path.value();
        ensure!(
            path.len() <= 4096 &&
                !path.starts_with('/') &&
                path.split('/').all(|part| !matches!(part, "" | "." | "..")),
            "invalid composed fragment source path"
        );
        ensure!(
            catalog.sources.len() <
                crate::publication::limits::LINK_INDEX_ENTRIES,
            "website source aliases exceed budget"
        );
        if let Some(previous) =
            catalog.sources.insert(path.into(), owner.as_str().into())
        {
            ensure!(
                previous == owner.as_str(),
                "ambiguous composed source alias: {path}"
            );
        }
    }
    Ok(())
}
