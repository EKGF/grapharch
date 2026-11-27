//! The website pass: authored pages in placement order, then generated
//! reports, selected assets, search and route aliases.
use {
    super::{
        budget,
        catalog::Catalog,
        header::Header,
        header_links,
        interactive_svg,
        metadata,
        navigation,
        page::PageWriter,
        redirects,
        report::Report,
        repository,
        search,
        site_chrome,
        theme,
    },
    crate::publication::render::{
        content::Entries,
        model::{Plan, title},
    },
    anyhow::{Result, ensure},
    std::{collections::HashMap, path::Path, time::Instant},
};

pub(in crate::publication::render) async fn write(
    plan: &Plan,
    stage: &Path,
) -> Result<Vec<String>> {
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
    let chrome = site_chrome::Options::read(plan).await?;
    let tokens = site_chrome::ThemeTokens::read(plan).await?;
    let theme_tokens = tokens.css(theme.root())?;
    let status_band =
        site_chrome::StatusBand::read(plan, tokens.flag("statusRibbon")?)
            .await?;
    let mut files = vec![
        "style.css".to_owned(),
        ".nojekyll".into(),
        "404.html".into(),
    ];
    files.extend(interactive_files);
    let keywords = plan
        .conformance
        .css(plan, crate::publication::theme::Builtin::Reader)
        .await?;
    tokio::fs::write(
        stage.join("style.css"),
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{keywords}\n{theme_tokens}",
            include_str!("style.css"),
            concat!(
                include_str!("../callouts.css"),
                include_str!("../diagrams.css"),
                include_str!("../inline-code.css")
            ),
            include_str!("../provenance.css"),
            theme.css(),
            include_str!("documentation.css"),
            include_str!("search.css"),
            include_str!("gallery.css"),
            include_str!("site_footer.css")
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
        repository: repository::link(plan, theme).await?,
        theme,
        links: header_links::Links::read(plan).await?,
        metadata: metadata::Metadata::read(plan).await?,
        navigation: navigation::Navigations::read(plan, &catalog).await?,
        chrome,
        footer: site_chrome::SiteFooter::read(plan).await?,
        status_band,
    };
    header.metadata.indexes(stage, &catalog, &mut files).await?;
    let pages = PageWriter { plan, stage, catalog: &catalog, header: &header };
    let mut diagrams = false;
    let mut entries = Entries::new(plan).await?;
    // bounded: one page's selected fragments, never the complete site's bodies.
    let mut body = String::new();
    let mut current = None;
    let mut fragments = Vec::new();
    let mut seen = HashMap::new();
    while let Some(entry) = entries.next(plan).await? {
        diagrams |= crate::publication::render::diagram_notes::contains(
            &entry.body,
            &entry.format,
        );
        let owner = catalog.owner(&entry.fragment)?;
        if current.as_ref().is_some_and(|page| page != &owner) {
            let page = current.take().unwrap();
            pages.write(&page, &body, &fragments, &mut files).await?;
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
        pages.write(&page, &body, &fragments, &mut files).await?;
    }
    tracing::debug!(
        phase = "website-authored-pages",
        elapsed_ms = started.elapsed().as_millis()
    );
    if diagrams {
        ensure!(
            !catalog.pages.values().any(|page| {
                page.route == crate::publication::render::diagram_notes::ROUTE
            }),
            "diagram note route conflicts with a selected asset"
        );
        Report {
            slug:             "diagram-notes",
            title:            "Diagram interpretation",
            body:             crate::publication::render::diagram_notes::html()
                .as_bytes(),
            navigation_owner: None,
            breadcrumb_root:  None,
        }
        .write(plan, stage, &catalog, &header)
        .await?;
        files.push(crate::publication::render::diagram_notes::ROUTE.into());
    }
    if !plan.quality.is_empty() {
        ensure!(
            !files.iter().any(|route| route == "quality-issues.html"),
            "authored page conflicts with the quality report route"
        );
        // bounded: quality evidence is already limited by the publication
        // diagnostic budget.
        let mut body = Vec::new();
        crate::publication::render::quality::html_heading(
            &mut body,
            &plan.quality,
            "Quality issues",
            &plan.quality_evidence,
            1,
        )
        .await?;
        Report {
            slug:             "quality-issues",
            title:            "Quality issues",
            body:             &body,
            navigation_owner: None,
            breadcrumb_root:  None,
        }
        .write(plan, stage, &catalog, &header)
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
    Report {
        slug:             "provenance",
        title:            "Document provenance",
        body:             &body,
        navigation_owner: None,
        breadcrumb_root:  None,
    }
    .write(plan, stage, &catalog, &header)
    .await?;
    files.push("provenance.html".into());
    pages.write_assets(&mut files).await?;
    if header.chrome.search() {
        search::write(plan, &catalog, stage, &mut files).await?;
    }
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
