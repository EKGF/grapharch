//! Shared companion website shell for authored pages and generated reports.
use super::{
    super::{markup::html_escape as esc, model::Plan},
    Header,
};

pub(super) struct Page<'a> {
    pub report:         bool,
    pub title:          &'a str,
    pub id:             &'a str,
    pub fragment_count: usize,
    pub prefix:         &'a str,
    pub navigation:     &'a str,
    pub breadcrumbs:    &'a str,
    pub heading:        &'a str,
    pub body:           &'a str,
    pub gallery:        &'a str,
    pub previous_next:  &'a str,
    pub outline:        &'a str,
}

pub(super) fn render(plan: &Plan, header: &Header, page: Page<'_>) -> String {
    let project_title = &header.title;
    let repository = &header.repository;
    let Page {
        prefix,
        navigation,
        breadcrumbs,
        heading,
        gallery,
        previous_next,
        outline: page_outline,
        ..
    } = page;
    let project_page = header.theme.project() && !page.report;
    let layout = if project_page {
        "project-layout"
    } else if page.report {
        "site-layout report-layout"
    } else if gallery.is_empty() {
        "site-layout"
    } else {
        "site-layout gallery-layout"
    };
    let quality_link = if plan.quality.is_empty() {
        String::new()
    } else {
        format!(
            "<p><a href=\"{prefix}quality-issues.html\">Quality issues</a></p>"
        )
    };
    let generator_credit = super::super::credit::HTML;
    let organization_identity = header.theme.identity();
    let header_links =
        header
            .theme
            .links(repository, prefix, &header.links.html(prefix));
    let project_brand = format!(
        "<a class=\"brand\" href=\"{prefix}\">{}<span \
         class=\"brand-version\"><span class=\"visually-hidden\">Version \
         </span>{}</span><span \
         class=\"brand-context\">Documentation</span></a>",
        esc(project_title),
        esc(env!("CARGO_PKG_VERSION"))
    );
    let header_content = if header.theme.community() {
        format!(
            "<div class=\"community-header-row\"\
             >{organization_identity}{header_links}</div><div \
             class=\"project-identity-row\">{project_brand}</div>"
        )
    } else {
        format!("{project_brand}{header_links}")
    };
    let identity_attribution = header.theme.attribution(prefix);
    let community_footer = header.theme.community_footer(prefix);
    let article_footer = if community_footer.is_empty() {
        format!(
            "<footer>{generator_credit} · <a \
             href=\"{prefix}provenance.html\">Provenance</a> · <a \
             href=\"{prefix}\">Documentation \
             home</a>{identity_attribution}</footer>"
        )
    } else {
        String::new()
    };

    let neighbors = if previous_next.is_empty() {
        String::new()
    } else {
        format!(
            "<nav class=\"page-neighbors\" aria-label=\"Adjacent \
             topics\">{previous_next}</nav>"
        )
    };
    let navigation_panel = if project_page {
        format!(
            "<nav class=\"project-navigation\" \
             aria-label=\"Project\">{navigation}</nav>"
        )
    } else {
        format!(
            "<aside class=\"site-navigation\" data-project=\"{}\" \
             data-target=\"{}\"><details open><summary>Browse \
             documentation</summary><nav \
             aria-label=\"Documentation\">{navigation}</nav></details></aside>",
            esc(plan.project.as_str()),
            esc(plan.target.as_str())
        )
    };
    let body = if project_page {
        super::project::sections(page.body)
    } else {
        page.body.into()
    };
    let trail = if project_page {
        String::new()
    } else {
        format!(
            "<div class=\"breadcrumbs\"><a href=\"{prefix}\">Docs</a><span \
             aria-hidden=\"true\">/</span>{breadcrumbs}<span>{}</span></div>",
            esc(page.title)
        )
    };
    let neighbors = if project_page {
        String::new()
    } else {
        neighbors
    };
    let page_outline = if project_page { "" } else { page_outline };
    let body_attribute = header.theme.body_attribute();
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{page_title} · {project_title}</title><meta name="generator" content="Grapharch"><meta name="grapharch-revision" content="{revision}"><meta name="grapharch-target" content="{target}"><meta name="grapharch-page" content="{id}"><meta name="grapharch-fragment-count" content="{fragment_count}"><link rel="stylesheet" href="{prefix}style.css"><link rel="icon" href="data:,"></head>
<body{body_attribute}><a class="skip-link" href="#main-content">Skip to content</a>
<header class="site-header">{header_content}</header>
<div class="{layout}">{navigation_panel}
<main id="main-content" tabindex="-1">{trail}<article>{heading}{body}{gallery}</article>{neighbors}{quality_link}{article_footer}</main>
{page_outline}</div>{community_footer}<script>{navigation_script}</script></body></html>"##,
        page_title = esc(page.title),
        project_title = esc(project_title),
        revision = esc(plan.revision.as_str()),
        target = esc(plan.target.as_str()),
        navigation_script = include_str!("nav.js"),
        id = esc(page.id),
        fragment_count = page.fragment_count,
        body = body,
    )
}
