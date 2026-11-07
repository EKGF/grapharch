use {
    super::{lit, q, title},
    crate::publication::{dataset::identity, term, writer::ContributionStream},
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    serde_yaml::Value,
    std::path::{Component, Path},
};

pub(super) async fn write(
    stream: &mut ContributionStream,
    profile: &NamedNode,
    root: &Path,
    nav: &Value,
) -> Result<()> {
    let sidebar = identity("mkdocs-navigation", &[
        profile.as_str(),
        "sidebar",
    ]);
    let primary = identity("mkdocs-navigation", &[
        profile.as_str(),
        "primary",
    ]);
    for (tree, region) in [(&sidebar, "sidebar"), (&primary, "primary")] {
        for quad in [
            q(profile, "navigation", tree.clone()),
            q(tree, "type", term("Navigation")),
            q(tree, "navigationRegion", lit(region)),
            title(tree, "Documentation"),
        ] {
            stream.write(quad).await?;
        }
    }
    let mut pending = vec![(nav, None::<NamedNode>, String::new(), 0usize)]; // bounded: 512 entries, depth 16 validated in YAML.
    let mut count = 0;
    while let Some((items, parent, prefix, depth)) = pending.pop() {
        ensure!(depth <= 16, "MkDocs navigation exceeds depth 16");
        let items = items.as_sequence().ok_or_else(|| {
            anyhow::anyhow!("MkDocs nav sections must be lists")
        })?;
        ensure!(
            !items.is_empty(),
            "MkDocs nav sections cannot be empty"
        );
        for (order, item) in items.iter().enumerate() {
            count += 1;
            ensure!(
                count <= 512,
                "MkDocs navigation exceeds 512 entries"
            );
            let (label, value) = if let Some(map) = item.as_mapping() {
                ensure!(map.len() == 1, "nav entry must have one label");
                let (key, value) = map.iter().next().unwrap();
                (
                    Some(key.as_str().ok_or_else(|| {
                        anyhow::anyhow!("nav label must be text")
                    })?),
                    value,
                )
            } else {
                (None, item)
            };
            let position = format!("{prefix}/{order}");
            let entry = identity("mkdocs-navigation-entry", &[
                profile.as_str(),
                &position,
            ]);
            for quad in [
                q(&sidebar, "navigationEntry", entry.clone()),
                q(&primary, "navigationEntry", entry.clone()),
                q(&entry, "type", term("NavigationEntry")),
                q(
                    &entry,
                    "order",
                    oxrdf::Literal::from(order as u32),
                ),
            ] {
                stream.write(quad).await?;
            }
            if let Some(parent) = &parent {
                stream
                    .write(q(&entry, "parentEntry", parent.clone()))
                    .await?;
            }
            if let Some(label) = label {
                ensure!(!label.is_empty(), "nav labels cannot be empty");
                stream.write(title(&entry, label)).await?;
            }
            if value.is_sequence() {
                ensure!(label.is_some(), "nav section needs a label");
                pending.push((value, Some(entry), position, depth + 1));
                continue;
            }
            let path = value.as_str().ok_or_else(|| {
                anyhow::anyhow!("nav destination must be a path or URL")
            })?;
            if let Ok(url) = url::Url::parse(path) {
                ensure!(
                    matches!(url.scheme(), "https" | "http" | "mailto"),
                    "unsafe nav URL"
                );
                stream.write(q(&entry, "url", lit(path))).await?;
                if label.is_none() {
                    stream.write(title(&entry, path)).await?;
                }
                continue;
            }
            let (path, anchor) = path
                .split_once('#')
                .map_or((path, None), |(p, a)| (p, Some(a)));
            ensure!(
                !path.is_empty() &&
                    !path.contains('?') &&
                    !Path::new(path)
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_))),
                "nav path must be relative to docs_dir: {path}"
            );
            let file = root.join(path);
            let canonical =
                tokio::fs::canonicalize(&file).await.map_err(|error| {
                    anyhow::anyhow!("nav destination {path}: {error}")
                })?;
            ensure!(
                canonical.starts_with(root) &&
                    canonical.is_file() &&
                    canonical.extension().is_some_and(|v| v == "md"),
                "nav destination must be a contained Markdown file: {path}"
            );
            stream.write(q(&entry, "sourcePath", lit(path))).await?;
            if let Some(anchor) = anchor {
                ensure!(!anchor.is_empty(), "nav anchor cannot be empty");
                stream
                    .write(q(&entry, "headingAnchor", lit(anchor)))
                    .await?;
            }
        }
    }
    Ok(())
}
