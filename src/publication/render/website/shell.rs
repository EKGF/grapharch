//! Shared companion website shell for authored pages and generated reports.
use super::{
    super::{markup::html_escape as esc, model::Plan},
    Header,
};

pub(super) struct Page<'a> {
    pub report:           bool,
    pub panels:           &'a [String],
    pub presentation:     &'a super::project::Presentation,
    pub extra_head:       &'a str,
    pub title:            &'a str,
    pub id:               &'a str,
    pub navigation_owner: Option<&'a str>,
    pub fragment_count:   usize,
    pub prefix:           &'a str,
    pub navigation:       &'a str,
    pub breadcrumbs:      &'a str,
    pub heading:          &'a str,
    pub body:             &'a str,
    pub gallery:          &'a str,
    pub previous_next:    &'a str,
    pub outline:          &'a str,
}

pub(super) fn render(plan: &Plan, header: &Header, page: Page<'_>) -> String {
    let active = page.navigation_owner.unwrap_or(page.id);
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
    let project_page = (header.theme.project() ||
        page.presentation.explicit_hero) &&
        !page.report;
    let section_sidebar = header.navigation.html("sidebar", active, prefix);
    let without_sidebar =
        header.navigation.sectioned() && section_sidebar.is_none();
    let layout = if without_sidebar && !gallery.is_empty() {
        "site-layout gallery-layout gallery-home"
    } else if without_sidebar {
        "site-layout section-landing"
    } else if project_page {
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
    let generator_credit = super::super::credit::html();
    let organization_identity = header.theme.identity();
    let header_links = header.theme.links(
        repository,
        prefix,
        &header.links.html(prefix),
        super::search::button(),
    );
    let brand_context = if header.navigation.sectioned() {
        ""
    } else {
        "<span class=\"brand-context\">Documentation</span>"
    };
    let project_brand = format!(
        "<a class=\"brand\" href=\"{prefix}\">{}<span class=\"brand-version\" \
         data-build-channel=\"{}\" data-generator-revision=\"{}\" \
         data-build-number=\"{}\" title=\"{}\"><span \
         class=\"visually-hidden\">Version </span>{}</span>{brand_context}</a>",
        esc(project_title),
        esc(env!("GRAPHARCH_BUILD_CHANNEL")),
        esc(env!("GRAPHARCH_GENERATOR_REVISION")),
        esc(env!("GRAPHARCH_BUILD_NUMBER")),
        esc(env!("GRAPHARCH_BUILD_DETAILS")),
        esc(env!("GRAPHARCH_BUILD_LABEL"))
    );
    let header_content =
        if header.navigation.documentation && header.theme.community() {
            format!(
                "<div class=\"community-header-row\"\
                 >{organization_identity}{header_links}</div>"
            )
        } else if header.navigation.documentation {
            header_links
        } else if header.theme.project() {
            format!(
                "<div class=\"community-header-row\"\
                 >{organization_identity}{header_links}</div>"
            )
        } else if header.theme.community() {
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
    let explicit_sidebar = section_sidebar;
    let primary_navigation = header
        .navigation
        .html("primary", active, prefix)
        .unwrap_or_default();
    let primary_navigation = if header.navigation.sectioned() {
        format!(
            "<div class=\"project-identity-row \
             documentation-product-row\">{project_brand}{primary_navigation}</\
             div>"
        )
    } else if header.navigation.documentation {
        format!(
            "<div class=\"project-identity-row documentation-product-row\"><a \
             class=\"documentation-title\" \
             href=\"{prefix}\">{}</a>{primary_navigation}</div>",
            esc(project_title)
        )
    } else {
        primary_navigation
    };
    let footer_navigation = header
        .navigation
        .html("footer", active, prefix)
        .unwrap_or_default();
    let navigation_panel = if without_sidebar ||
        (project_page && header.navigation.documentation)
    {
        String::new()
    } else if let Some(sidebar) = explicit_sidebar {
        format!(
            "<aside class=\"site-navigation\" data-project=\"{}\" \
             data-target=\"{}\"><details open><summary>Browse \
             documentation</summary>{sidebar}</details></aside>",
            esc(plan.project.as_str()),
            esc(plan.target.as_str())
        )
    } else if project_page {
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
    let search_dialog = super::search::dialog(prefix);
    let body = if project_page {
        super::project::sections(
            &format!("{heading}{}", page.body),
            page.panels,
            page.presentation,
        )
    } else {
        page.body.into()
    };
    let heading = if project_page { "" } else { heading };
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
    let prepaint = header.theme.prepaint();
    let body_attribute = if project_page && !header.theme.project() {
        if header.theme.community() {
            " class=\"theme-ekgf project-website\""
        } else {
            " class=\"project-website\""
        }
    } else {
        header.theme.body_attribute()
    };
    let layout_attribute = if header.navigation.documentation {
        " data-layout=\"documentation\""
    } else {
        ""
    };
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{page_title} · {project_title}</title><meta name="generator" content="Grapharch"><meta name="grapharch-revision" content="{revision}"><meta name="grapharch-target" content="{target}"><meta name="grapharch-page" content="{id}"><meta name="grapharch-fragment-count" content="{fragment_count}">{prepaint}{extra_head}<link rel="stylesheet" href="{prefix}style.css"><link rel="icon" href="data:,"></head>
<body{body_attribute}{layout_attribute}><a class="skip-link" href="#main-content">Skip to content</a>
<header class="site-header">{header_content}{primary_navigation}</header>
<div class="{layout}">{navigation_panel}
<main id="main-content" tabindex="-1">{trail}<article>{heading}{body}{gallery}</article>{neighbors}{quality_link}{article_footer}</main>
{page_outline}</div>{footer_navigation}{community_footer}{search_dialog}<script>{navigation_script}</script></body></html>"##,
        extra_head = page.extra_head,
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
