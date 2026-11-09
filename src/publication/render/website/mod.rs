//! A website is an arrangement of selected logical fragments, with page and
//! navigation metadata read exclusively from the pinned documentation graph.
mod artifact;
mod budget;
pub(super) const MAX_ARTIFACTS: usize = budget::MAX_ARTIFACTS;
mod catalog;
mod gallery;
mod header_links;
mod interactive_svg;
mod markdown;
mod metadata;
mod navigation;
mod project;
mod redirects;
mod repository;
mod search;
mod shell;
mod theme;
use {
    super::{
        content::Entries,
        markup::html_escape as esc,
        model::{Plan, title},
    },
    anyhow::{Result, ensure},
    catalog::Catalog,
    std::{collections::HashMap, path::Path, time::Instant},
};

struct Report<'a> {
    slug:             &'a str,
    title:            &'a str,
    body:             &'a [u8],
    navigation_owner: Option<&'a str>,
}

struct Header {
    title:      String,
    repository: String,
    theme:      theme::Theme,
    links:      header_links::Links,
    metadata:   metadata::Metadata,
    navigation: navigation::Navigations,
}

pub(super) async fn write(plan: &Plan, stage: &Path) -> Result<Vec<String>> {
    let started = Instant::now();
    let mut catalog = Catalog::read(plan).await?;
    tracing::debug!(
        phase = "website-catalog",
        elapsed_ms = started.elapsed().as_millis(),
        routes = catalog.pages.len()
    );
    let interactive_files =
        interactive_svg::prepare(plan, &mut catalog, stage).await?;
    tracing::debug!(
        phase = "website-interactive-projections",
        elapsed_ms = started.elapsed().as_millis(),
        files = interactive_files.len()
    );
    let artifacts = budget::ArtifactBudget::new(
        catalog.pages.values().filter(|page| !page.asset).count(),
        catalog.pages.values().filter(|page| page.asset).count(),
        interactive_files.len(),
    )?;
    let theme = theme::Theme::read(plan).await?;
    let mut files = vec![
        "style.css".to_owned(),
        ".nojekyll".into(),
        "404.html".into(),
    ];
    files.extend(interactive_files);
    tokio::fs::write(
        stage.join("style.css"),
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}",
            include_str!("style.css"),
            concat!(
                include_str!("../callouts.css"),
                include_str!("../diagrams.css")
            ),
            include_str!("../provenance.css"),
            theme.css(),
            include_str!("documentation.css"),
            include_str!("search.css"),
            include_str!("gallery.css")
        ),
    )
    .await?;
    theme.assets(stage, &mut files).await?;
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
        title: title(&plan.source, &plan.project).await?,
        repository: repository::link(plan).await?,
        theme,
        links: header_links::Links::read(plan).await?,
        metadata: metadata::Metadata::read(plan).await?,
        navigation: navigation::Navigations::read(plan, &catalog).await?,
    };
    header.metadata.indexes(stage, &catalog, &mut files).await?;
    let mut diagrams = false;
    let mut entries = Entries::new(plan).await?;
    // bounded: one page's selected fragments, never the complete site's bodies.
    let mut body = String::new();
    let mut current = None;
    let mut fragments = Vec::new();
    let mut seen = HashMap::new();
    while let Some(entry) = entries.next(plan).await? {
        diagrams |= super::diagram_notes::contains(&entry.body, &entry.format);
        let owner = catalog.owner(&entry.fragment)?;
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
    tracing::debug!(
        phase = "website-authored-pages",
        elapsed_ms = started.elapsed().as_millis()
    );
    if diagrams {
        ensure!(
            !catalog
                .pages
                .values()
                .any(|page| page.route == super::diagram_notes::ROUTE),
            "diagram note route conflicts with a selected asset"
        );
        write_report(plan, stage, &catalog, &header, Report {
            slug:             "diagram-notes",
            title:            "Diagram interpretation",
            body:             super::diagram_notes::html().as_bytes(),
            navigation_owner: None,
        })
        .await?;
        files.push(super::diagram_notes::ROUTE.into());
    }
    if !plan.quality.is_empty() {
        ensure!(
            !files.iter().any(|route| route == "quality-issues.html"),
            "authored page conflicts with the quality report route"
        );
        // bounded: quality evidence is already limited by the publication
        // diagnostic budget.
        let mut body = Vec::new();
        super::quality::html_heading(
            &mut body,
            &plan.quality,
            "Quality issues",
            &plan.quality_evidence,
            1,
        )
        .await?;
        write_report(plan, stage, &catalog, &header, Report {
            slug:             "quality-issues",
            title:            "Quality issues",
            body:             &body,
            navigation_owner: None,
        })
        .await?;
        files.push("quality-issues.html".into());
    }
    ensure!(
        !files.iter().any(|route| route == "provenance.html"),
        "authored page conflicts with provenance report route"
    );
    // bounded: provenance has at most 128 repositories and five recent commits
    // each.
    let mut body = Vec::new();
    plan.provenance
        .html_heading(&mut body, "Document provenance", 1)
        .await?;
    write_report(plan, stage, &catalog, &header, Report {
        slug:             "provenance",
        title:            "Document provenance",
        body:             &body,
        navigation_owner: None,
    })
    .await?;
    files.push("provenance.html".into());
    let mut assets = plan.assets().await?;
    let mut asset_bytes = 0u64;
    let mut image_bytes = 0u64;
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
        let image = plan.source.optional(id, "format").await?;
        if let Some(oxrdf::Term::Literal(mime)) = &image &&
            mime.value().starts_with("image/")
        {
            let size: u64 =
                plan.source.text(id, "contentSize").await?.parse()?;
            ensure!(
                size <= crate::publication::image_assets::MAX_IMAGE_BYTES
                    as u64,
                "image exceeds byte budget"
            );
            image_bytes += size;
            ensure!(image_bytes <= crate::publication::image_assets::MAX_PUBLICATION_IMAGE_BYTES as u64, "publication images exceed total byte budget");
        }
        let output = stage.join(&asset.route);
        tokio::fs::create_dir_all(
            output
                .parent()
                .ok_or_else(|| anyhow::anyhow!("artifact has no parent"))?,
        )
        .await?;
        artifact::write(plan, id, &output).await?;
        if let Some(oxrdf::Term::Literal(mime)) = image &&
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
    if let Some(job) = crate::publication::document::publication_inventory::model::configured_job(&plan.dataset, &plan.source, &plan.project).await? {
        ensure!(!files.iter().any(|path| path == "publication-versions.json"), "publication inventory artifact collides with authored content");
        let inventory = crate::publication::document::publication_inventory::model::Inventory::read(&plan.dataset, &plan.source, &job.subject).await?;
        tokio::fs::write(stage.join("publication-versions.json"), serde_json::to_vec_pretty(&inventory)?).await?;
        files.push("publication-versions.json".into());
    }
    search::write(plan, &catalog, stage, &mut files).await?;
    let before_aliases = files.len();
    redirects::write(plan, stage, &catalog, &mut files).await?;
    artifacts.validate(files.len() - before_aliases, files.len())?;
    tracing::debug!(
        phase = "website-complete",
        elapsed_ms = started.elapsed().as_millis(),
        files = files.len()
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
        let first = fragments
            .first()
            .ok_or_else(|| anyhow::anyhow!("website page has no fragments"))?;
        let last = fragments.last().unwrap();
        let contextual =
            super::content_blocks::enclosing(plan, first, last, body).await?;
        let content = markdown::render(&contextual, page, catalog)?;
        let prefix = "../".repeat(page.route.matches('/').count());
        let navigation = catalog.navigation(id, &prefix)?;
        let gallery = gallery::render(plan, catalog, id, &prefix).await?;
        for detail in &gallery.details {
            let route = format!("{}.html", detail.slug);
            ensure!(
                !files.contains(&route) &&
                    !catalog.pages.values().any(|p| p.route == route),
                "showcase detail route collision: {route}"
            );
            write_report(plan, stage, catalog, header, Report {
                slug:             &detail.slug,
                title:            &detail.title,
                body:             detail.body.as_bytes(),
                navigation_owner: Some(id.as_str()),
            })
            .await?;
            files.push(route);
        }
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
        let mut breadcrumbs = String::new();
        for ancestor in catalog.ancestors(id)? {
            breadcrumbs.push_str(&format!(
                "<a href=\"{prefix}{}\">{}</a><span \
                 aria-hidden=\"true\">/</span>",
                esc(&ancestor.route),
                esc(&ancestor.title)
            ));
        }
        let previous_next = if let Some((authored_breadcrumbs, neighbors)) =
            header.navigation.context(id.as_str(), &prefix)
        {
            breadcrumbs = authored_breadcrumbs;
            neighbors
        } else {
            catalog.previous_next(id, &prefix)?
        };
        let heading = if content.headings.first().is_some_and(|h| h.level == 1)
        {
            String::new()
        } else {
            format!("<h1>{}</h1>", esc(&page.title))
        };
        let extra_head =
            header.metadata.head(plan, &header.title, page).await?;
        let panels = project::panels(plan, id).await?;
        let presentation = project::Presentation::read(plan, id).await?;
        for anchor in
            presentation.features.iter().chain(&presentation.resources)
        {
            ensure!(
                content
                    .headings
                    .iter()
                    .any(|h| h.level == 2 && h.anchor == *anchor),
                "presentation section {anchor} is not a level-two heading"
            );
        }
        for anchor in presentation.icons.keys() {
            ensure!(
                content
                    .headings
                    .iter()
                    .any(|h| matches!(h.level, 2 | 3) && h.anchor == *anchor),
                "card icon {anchor} is not a level-two or level-three heading"
            );
        }
        for anchor in &panels {
            ensure!(
                content.headings.iter().any(|heading| {
                    heading.level == 2 && heading.anchor == *anchor
                }),
                "panel section {anchor} is not a level-two page heading"
            );
        }
        let html = shell::render(plan, header, shell::Page {
            panels:           &panels,
            presentation:     &presentation,
            extra_head:       &extra_head,
            report:           false,
            title:            &page.title,
            id:               id.as_str(),
            navigation_owner: None,
            fragment_count:   fragments.len(),
            prefix:           &prefix,
            navigation:       &navigation,
            breadcrumbs:      &breadcrumbs,
            heading:          &heading,
            body:             &content.html,
            gallery:          &gallery.html,
            previous_next:    &previous_next,
            outline:          &page_outline,
        });
        tokio::fs::write(output, html).await?;
    }
    files.push(path);
    Ok(())
}

async fn write_report(
    plan: &Plan,
    stage: &Path,
    catalog: &Catalog,
    header: &Header,
    report: Report<'_>,
) -> Result<()> {
    let Report { slug, title, body, navigation_owner } = report;
    let id = crate::publication::dataset::identity("website-report", &[
        plan.target.as_str(),
        slug,
    ]);
    let navigation = catalog.report_navigation(&id, "./");
    let content = std::str::from_utf8(body)?;
    let html = shell::render(plan, header, shell::Page {
        report: true,
        panels: &[],
        presentation: &project::Presentation::default(),
        extra_head: "<meta name=\"robots\" content=\"noindex,follow\">",
        title,
        id: id.as_str(),
        navigation_owner,
        fragment_count: 0,
        prefix: "./",
        navigation: &navigation,
        breadcrumbs: "",
        heading: "",
        body: content,
        gallery: "",
        previous_next: "",
        outline: "",
    });
    tokio::fs::write(stage.join(format!("{slug}.html")), html).await?;
    Ok(())
}
