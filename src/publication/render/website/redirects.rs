//! Explicit retained route aliases preserve published URLs on static hosts.
use {
    super::{catalog::Catalog, esc},
    crate::publication::render::model::{Plan, node},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::{
        collections::{BTreeMap, HashSet},
        path::Path,
    },
};
struct Destination {
    owner:  NamedNode,
    anchor: Option<String>,
}
pub(super) async fn write(
    plan: &Plan,
    stage: &Path,
    catalog: &Catalog,
    files: &mut Vec<String>,
) -> Result<()> {
    let mut aliases = BTreeMap::new(); // bounded: at most 2048 explicit publication aliases.
    let mut seen = HashSet::new(); // bounded: selected pages and fragments, same link-index budget.
    let mut entries = plan.entries().await?;
    while let Some(row) = entries.next().await? {
        let fragment = node(&row["fragment"])?;
        let owner = catalog.owner(plan, fragment).await?;
        for resource in [fragment, &owner] {
            if !seen.insert(resource.clone()) {
                continue;
            }
            ensure!(
                seen.len() <= crate::publication::limits::LINK_INDEX_ENTRIES,
                "route alias subject index exceeds budget"
            );
            let mut rows = plan.source.objects(resource, "routeAlias").await?;
            let mut count = 0;
            while let Some(row) = rows.next().await? {
                count += 1;
                ensure!(count <= 64, "resource exceeds 64 route aliases");
                let Term::Literal(value) = &row["value"] else {
                    anyhow::bail!("route alias must be a literal")
                };
                ensure!(
                    value.value().len() <= 2048,
                    "route alias exceeds 2048 bytes"
                );
                let route = crate::publication::collect::page::route(
                    "index.md",
                    Some(value.value()),
                )?;
                ensure!(
                    !catalog.routes.contains_key(&route),
                    "route alias collides with a published page or asset: \
                     {route}"
                );
                let anchor = if resource == &owner {
                    None
                } else {
                    Some(
                        catalog
                            .fragment_anchors
                            .get(resource.as_str())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "aliased fragment has no selected \
                                     heading: {resource}"
                                )
                            })?
                            .clone(),
                    )
                };
                ensure!(
                    aliases.len() < super::budget::ALIASES,
                    "website exceeds 2048 route aliases"
                );
                ensure!(
                    aliases
                        .insert(route.clone(), Destination {
                            owner: owner.clone(),
                            anchor
                        })
                        .is_none(),
                    "duplicate route alias: {route}"
                );
            }
        }
    }
    for (route, destination) in aliases {
        let file = format!("{route}index.html");
        super::super::files::validate_artifact_path(&file)?;
        for other in files
            .iter()
            .map(String::as_str)
            .chain(std::iter::once("manifest.yaml"))
        {
            ensure!(
                file != other &&
                    !file.starts_with(&format!("{other}/")) &&
                    !other.starts_with(&format!("{file}/")),
                "route alias collides with publication artifact: {file}"
            );
        }
        let page = &catalog.pages[destination.owner.as_str()];
        ensure!(
            !page.asset,
            "route alias cannot target a binary or text artifact"
        );
        if let Some(anchor) = &destination.anchor {
            ensure!(
                page.headings.contains(anchor),
                "route alias heading is not published: {anchor}"
            );
        }
        let prefix = "../"
            .repeat(route.split('/').filter(|part| !part.is_empty()).count());
        let target = format!(
            "{prefix}{}{}",
            page.route,
            destination
                .anchor
                .as_ref()
                .map(|anchor| format!("#{anchor}"))
                .unwrap_or_default()
        );
        let quoted = serde_json::to_string(&target)?;
        let escaped = esc(&target);
        let html = format!(
            "<!doctype html><html lang=\"en\"><head><meta \
             charset=\"utf-8\"><meta name=\"viewport\" \
             content=\"width=device-width,initial-scale=1\"><meta \
             name=\"robots\" content=\"noindex\"><title>Page \
             moved</title><link rel=\"canonical\" \
             href=\"{escaped}\"><script>const target=new \
             URL({quoted},location.href);if(location.hash)target.\
             hash=location.hash;if(location.search)target.search=location.\
             search;location.replace(target.href);</script><meta \
             http-equiv=\"refresh\" \
             content=\"1;url={escaped}\"></head><body><p>This content has \
             moved. <a href=\"{escaped}\">Continue to \
             {}</a>.</p></body></html>",
            esc(&page.title)
        );
        let path = stage.join(&file);
        tokio::fs::create_dir_all(
            path.parent().expect("alias file has parent"),
        )
        .await?;
        tokio::fs::write(path, html).await?;
        files.push(file);
    }
    Ok(())
}
