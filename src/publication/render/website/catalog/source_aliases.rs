//! Retained authored fragments can be composed into a differently routed page.
use {
    super::Catalog,
    crate::publication::render::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
pub(super) async fn retain(
    catalog: &mut Catalog,
    plan: &Plan,
    fragment: &NamedNode,
    owner: &NamedNode,
) -> Result<()> {
    if fragment == owner {
        return Ok(());
    }
    let mut paths = plan.source.objects(fragment, "sourcePath").await?;
    while let Some(row) = paths.next().await? {
        let Term::Literal(path) = &row["value"] else {
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
