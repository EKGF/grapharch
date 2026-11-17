//! One authored page: its selected fragments rendered inside the site shell.
use {
    super::{
        catalog::Catalog,
        gallery,
        header::Header,
        markdown,
        project,
        report::Report,
        shell,
    },
    crate::publication::render::{markup::html_escape as esc, model::Plan},
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::path::Path,
};

/// What every authored page of one website shares.
#[derive(Clone, Copy)]
pub(super) struct PageWriter<'a> {
    pub(super) plan:    &'a Plan,
    pub(super) stage:   &'a Path,
    pub(super) catalog: &'a Catalog,
    pub(super) header:  &'a Header,
}

impl PageWriter<'_> {
    /// Write the page owning `fragments`, whose concatenated Markdown is
    /// `body`, and record its output path in `files`.
    pub(super) async fn write(
        self,
        id: &NamedNode,
        body: &str,
        fragments: &[NamedNode],
        files: &mut Vec<String>,
    ) -> Result<()> {
        let Self { plan, stage, catalog, header } = self;
        let page = catalog.page(id)?;
        let path = if page.asset {
            page.route.clone()
        } else {
            format!("{}index.html", page.route)
        };
        let output = stage.join(&path);
        tokio::fs::create_dir_all(output.parent().ok_or_else(|| {
            anyhow::anyhow!("website artifact has no parent")
        })?)
        .await?;
        ensure!(
            !page.binary,
            "binary artifacts must be selected as plan assets"
        );
        if page.asset {
            tokio::fs::write(output, body).await?;
        } else {
            let first = fragments.first().ok_or_else(|| {
                anyhow::anyhow!("website page has no fragments")
            })?;
            let last = fragments.last().unwrap();
            let contextual =
                crate::publication::render::content_blocks::enclosing(
                    plan, first, last, body,
                )
                .await?;
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
                Report {
                    slug:             &detail.slug,
                    title:            &detail.title,
                    body:             detail.body.as_bytes(),
                    navigation_owner: Some(id.as_str()),
                    breadcrumb_root:  Some((
                        "Showcase",
                        &catalog.page(id)?.route,
                    )),
                }
                .write(plan, stage, catalog, header)
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
                     page\"><strong>On this \
                     page</strong>{outline}</nav></aside>"
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
            let heading =
                if content.headings.first().is_some_and(|h| h.level == 1) {
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
                    content.headings.iter().any(|h| {
                        matches!(h.level, 2 | 3) && h.anchor == *anchor
                    }),
                    "card icon {anchor} is not a level-two or level-three \
                     heading"
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
                breadcrumb_root:  None,
                fragment_count:   fragments.len(),
                prefix:           &prefix,
                navigation:       &navigation,
                breadcrumbs:      &breadcrumbs,
                heading:          &heading,
                body:             &content.html,
                gallery:          &gallery.html,
                previous_next:    &previous_next,
                outline:          &page_outline,
            })?;
            tokio::fs::write(output, html).await?;
        }
        files.push(path);
        Ok(())
    }
}
