//! A website is an arrangement of selected logical fragments, with page and
//! navigation metadata read exclusively from the pinned documentation graph.
mod artifact;
mod catalog;
mod gallery;
mod markdown;
mod repository;
use {
    super::{
        content::Entries,
        markup::html_escape as esc,
        model::{Plan, title},
    },
    anyhow::{Result, ensure},
    catalog::Catalog,
    std::{collections::HashMap, path::Path},
};

struct Header {
    title:      String,
    repository: String,
}

pub(super) async fn write(plan: &Plan, stage: &Path) -> Result<Vec<String>> {
    let catalog = Catalog::read(plan).await?;
    let mut files = vec![
        "style.css".to_owned(),
        ".nojekyll".into(),
        "404.html".into(),
    ];
    tokio::fs::write(
        stage.join("style.css"),
        format!(
            "{}\n{}",
            include_str!("style.css"),
            include_str!("../callouts.css")
        ),
    )
    .await?;
    tokio::fs::write(stage.join(".nojekyll"), "").await?;
    tokio::fs::write(
        stage.join("404.html"),
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta \
         name=\"viewport\" \
         content=\"width=device-width,initial-scale=1\"><title>Page not \
         found</title><h1>Page not found</h1><p>Use the documentation \
         navigation to find this topic.</p></html>",
    )
    .await?;
    let header = Header {
        title:      title(&plan.source, &plan.project).await?,
        repository: repository::link(plan).await?,
    };
    let mut entries = Entries::new(plan).await?;
    // bounded: one page's selected fragments, never the complete site's bodies.
    let mut body = String::new();
    let mut current = None;
    let mut fragments = Vec::new();
    let mut seen = HashMap::new();
    while let Some(entry) = entries.next(plan).await? {
        let owner = catalog.owner(plan, &entry.fragment).await?;
        if current.as_ref().is_some_and(|page| page != &owner) {
            let page = current.take().unwrap();
            write_page(
                plan, stage, &catalog, &page, &header, &body, &fragments,
                &mut files,
            )
            .await?;
            body.clear();
            fragments.clear();
        }
        if current.is_none() {
            ensure!(
                seen.insert(owner.clone(), ()).is_none(),
                "website page fragments must occupy contiguous placements"
            );
            current = Some(owner);
        }
        body.push_str(&entry.body);
        fragments.push(entry.fragment);
        ensure!(
            body.len() <= 1024 * 1024,
            "website page exceeds 1 MiB content budget"
        );
    }
    if let Some(page) = current {
        write_page(
            plan, stage, &catalog, &page, &header, &body, &fragments,
            &mut files,
        )
        .await?;
    }
    if !plan.quality.is_empty() {
        ensure!(
            !files.iter().any(|route| route == "quality-issues.html"),
            "authored page conflicts with the quality report route"
        );
        let mut file = tokio::io::BufWriter::new(
            tokio::fs::File::create(stage.join("quality-issues.html")).await?,
        );
        use tokio::io::AsyncWriteExt;
        file.write_all(b"<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Quality issues</title><link rel=\"stylesheet\" href=\"style.css\"></head><body><main>").await?;
        super::quality::html(&mut file, &plan.quality).await?;
        file.write_all(b"<p><a href=\"./\">Documentation home</a></p></main></body></html>").await?;
        file.flush().await?;
        files.push("quality-issues.html".into());
    }
    let mut assets = plan.assets().await?;
    let mut asset_bytes = 0u64;
    while let Some(row) = assets.next().await? {
        let id = super::model::node(&row["asset"])?;
        let asset = catalog.page(id)?;
        ensure!(
            asset.binary,
            "selected publication asset must be binary"
        );
        asset_bytes +=
            plan.source.text(id, "contentSize").await?.parse::<u64>()?;
        ensure!(
            asset_bytes <= crate::publication::limits::RETAINED_ASSET_BYTES,
            "website assets exceed 512 MiB budget"
        );
        let output = stage.join(&asset.route);
        tokio::fs::create_dir_all(
            output
                .parent()
                .ok_or_else(|| anyhow::anyhow!("artifact has no parent"))?,
        )
        .await?;
        artifact::write(plan, id, &output).await?;
        if let Some(oxrdf::Term::Literal(mime)) =
            plan.source.optional(id, "format").await? &&
            mime.value().starts_with("image/")
        {
            let size = tokio::fs::metadata(&output).await?.len();
            ensure!(
                size <= crate::publication::image_assets::MAX_IMAGE_BYTES
                    as u64,
                "image exceeds byte budget"
            );
            crate::publication::image_assets::validate(
                &tokio::fs::read(&output).await?,
                mime.value(),
            )?;
        }
        files.push(asset.route.clone());
    }
    ensure!(
        files.len() <= 512,
        "website exceeds 512-artifact budget"
    );
    Ok(files)
}

#[allow(clippy::too_many_arguments)]
async fn write_page(
    plan: &Plan,
    stage: &Path,
    catalog: &Catalog,
    id: &oxrdf::NamedNode,
    header: &Header,
    body: &str,
    fragments: &[oxrdf::NamedNode],
    files: &mut Vec<String>,
) -> Result<()> {
    let project_title = &header.title;
    let repository = &header.repository;
    let page = catalog.page(id)?;
    let path = if page.asset {
        page.route.clone()
    } else {
        format!("{}index.html", page.route)
    };
    let output = stage.join(&path);
    tokio::fs::create_dir_all(
        output
            .parent()
            .ok_or_else(|| anyhow::anyhow!("website artifact has no parent"))?,
    )
    .await?;
    ensure!(
        !page.binary,
        "binary artifacts must be selected as plan assets"
    );
    if page.asset {
        tokio::fs::write(output, body).await?;
    } else {
        let content = markdown::render(body, page, catalog)?;
        let prefix = "../".repeat(page.route.matches('/').count());
        let navigation = catalog.navigation(id, &prefix)?;
        let gallery = gallery::render(plan, catalog, id, &prefix).await?;
        let mut outline = String::new();
        for heading in &content.headings {
            if heading.level > 1 && heading.level <= 3 {
                outline.push_str(&format!(
                    "<a class=\"depth-{}\" href=\"#{}\">{}</a>",
                    heading.level,
                    esc(&heading.anchor),
                    esc(&heading.title)
                ));
            }
        }
        let page_outline = if outline.is_empty() {
            String::new()
        } else {
            format!(
                "<aside class=\"page-outline\"><nav aria-label=\"On this \
                 page\"><strong>On this page</strong>{outline}</nav></aside>"
            )
        };
        let layout = if gallery.is_empty() {
            "site-layout"
        } else {
            "site-layout gallery-layout"
        };
        let mut breadcrumbs = String::new();
        for ancestor in catalog.ancestors(id)? {
            breadcrumbs.push_str(&format!(
                "<a href=\"{prefix}{}\">{}</a><span \
                 aria-hidden=\"true\">/</span>",
                esc(&ancestor.route),
                esc(&ancestor.title)
            ));
        }
        let previous_next = catalog.previous_next(id, &prefix)?;
        let heading = if content.headings.first().is_some_and(|h| h.level == 1)
        {
            String::new()
        } else {
            format!("<h1>{}</h1>", esc(&page.title))
        };
        let quality_link = if plan.quality.is_empty() {
            String::new()
        } else {
            format!(
                "<p><a href=\"{prefix}quality-issues.html\">Quality \
                 issues</a></p>"
            )
        };
        let html = format!(
            r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{page_title} · {project_title}</title><meta name="generator" content="Grapharch"><meta name="grapharch-revision" content="{revision}"><meta name="grapharch-target" content="{target}"><meta name="grapharch-page" content="{id}"><meta name="grapharch-fragment-count" content="{fragment_count}"><link rel="stylesheet" href="{prefix}style.css"><link rel="icon" href="data:,"></head>
<body><a class="skip-link" href="#main-content">Skip to content</a>
<header class="site-header"><a class="brand" href="{prefix}">{project_title}<span>Documentation</span></a>{repository}</header>
<div class="{layout}"><aside class="site-navigation" data-project="{project}" data-target="{target}"><details open><summary>Browse documentation</summary><nav aria-label="Documentation">{navigation}</nav></details></aside>
<main id="main-content" tabindex="-1"><div class="breadcrumbs"><a href="{prefix}">Docs</a><span aria-hidden="true">/</span>{breadcrumbs}<span>{page_title}</span></div><article>{heading}{body}{gallery}</article><nav class="page-neighbors" aria-label="Adjacent topics">{previous_next}</nav>{quality_link}<footer>Generated by Grapharch from its documentation graph. · <a href="{prefix}">Documentation home</a></footer></main>
{page_outline}</div><script>{navigation_script}</script></body></html>"##,
            page_title = esc(&page.title),
            project_title = esc(project_title),
            revision = esc(plan.revision.as_str()),
            target = esc(plan.target.as_str()),
            project = esc(plan.project.as_str()),
            navigation_script = include_str!("nav.js"),
            id = esc(id.as_str()),
            fragment_count = fragments.len(),
            body = content.html,
        );
        tokio::fs::write(output, html).await?;
    }
    files.push(path);
    Ok(())
}
