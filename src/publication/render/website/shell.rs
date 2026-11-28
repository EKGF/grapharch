//! Shared companion website shell for authored pages and generated reports.
use super::{
    super::{markup::html_escape as esc, model::Plan},
    header::Header,
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
    pub breadcrumb_root:  Option<(&'a str, &'a str)>,
    pub breadcrumbs:      &'a str,
    pub heading:          &'a str,
    pub body:             &'a str,
    pub gallery:          &'a str,
    pub previous_next:    &'a str,
    pub outline:          &'a str,
    pub hidden:           super::page_regions::PageRegions,
}

pub(super) fn render(
    plan: &Plan,
    header: &Header,
    page: Page<'_>,
) -> anyhow::Result<String> {
    let active = page.navigation_owner.unwrap_or(page.id);
    let project_title = &header.title;
    // The chip names the documented package's version when one is known,
    // not the GraphArch build; the build stays in its data attributes.
    let package_version = header
        .software
        .as_ref()
        .and_then(super::software::Software::version);
    // A discovered project without one version (a workspace of separately
    // versioned crates) shows no chip rather than the generator's version.
    let chip_hidden = if header.software.is_some() && package_version.is_none()
    {
        " hidden"
    } else {
        ""
    };
    let facts = match &header.software {
        Some(software) if page.prefix.is_empty() && !page.report => {
            software.facts_html()
        },
        _ => String::new(),
    };
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
    let generator_credit = if header.chrome.generator_credit() {
        format!(
            "{} · <a href=\"{prefix}provenance.html\">Provenance</a> · ",
            super::super::credit::html()
        )
    } else {
        String::new()
    };
    let organization_identity = header.identity.brand_html(prefix);
    let identity_marks = header.identity.marks_html(prefix);
    let header_links = header.theme.links(
        header.identity.organization(),
        &header.links.html(prefix),
        super::theme::Controls {
            search: if header.chrome.search() {
                super::search::button()
            } else {
                ""
            },
            repository,
            marks: &identity_marks,
        },
    );
    // A header without organization identity or header links keeps its
    // controls on the product row, beside the title and sections.
    let community_row = header.theme.community() ||
        !header.identity.is_empty() ||
        !header.links.is_empty();
    let brand_context = if header.navigation.sectioned() {
        ""
    } else {
        "<span class=\"brand-context\">Documentation</span>"
    };
    let project_brand = format!(
        "<a class=\"brand\" href=\"{prefix}\">{}<span class=\"brand-version\" \
         data-build-channel=\"{}\" data-generator-revision=\"{}\" \
         data-build-number=\"{}\" data-build-attempt=\"{}\" \
         title=\"{}\"{chip_hidden}><span class=\"visually-hidden\">Version \
         </span>{}</span>{brand_context}</a>",
        esc(project_title),
        esc(env!("GRAPHARCH_BUILD_CHANNEL")),
        esc(env!("GRAPHARCH_GENERATOR_REVISION")),
        esc(env!("GRAPHARCH_BUILD_NUMBER")),
        esc(env!("GRAPHARCH_BUILD_ATTEMPT")),
        esc(&match package_version {
            Some(version) =>
                format!(
                    "Package version {version}; {}",
                    env!("GRAPHARCH_BUILD_DETAILS")
                ),
            None => env!("GRAPHARCH_BUILD_DETAILS").to_owned(),
        }),
        esc(&package_version
            .map(|version| format!("v{}", version.trim_start_matches('v')))
            .unwrap_or_else(|| env!("GRAPHARCH_BUILD_LABEL").to_owned()))
    );
    let header_content = if header.theme.project() ||
        (community_row && header.navigation.documentation)
    {
        format!(
            "<div class=\"community-header-row\"\
             >{organization_identity}{header_links}</div>"
        )
    } else if community_row {
        format!(
            "<div class=\"community-header-row\"\
             >{organization_identity}{header_links}</div><div \
             class=\"project-identity-row\">{project_brand}</div>"
        )
    } else if header.navigation.documentation {
        String::new()
    } else {
        format!("{project_brand}{header_links}")
    };
    let product_controls = if community_row { "" } else { &header_links };
    let header_content = if header.theme.documentation_identity() {
        super::documentation_header::DocumentationHeader {
            header,
            prefix,
            tabs: &if header.chrome.primary_navigation() {
                header
                    .navigation
                    .html_context("primary", page.id, active, prefix)
                    .unwrap_or_default()
            } else {
                String::new()
            },
        }
        .html()
    } else {
        header_content
    };
    let community_footer = match &header.footer {
        Some(footer) => {
            footer.html(prefix, header.chrome.generator_credit())?
        },
        None => String::new(),
    };
    let article_footer = if community_footer.is_empty() {
        format!(
            "<footer>{generator_credit}<a href=\"{prefix}\">Documentation \
             home</a></footer>"
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
    let primary_navigation = if header.chrome.primary_navigation() {
        header
            .navigation
            .html_context("primary", page.id, active, prefix)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let primary_navigation = if header.theme.documentation_identity() {
        String::new()
    } else if header.navigation.sectioned() {
        format!(
            "<div class=\"project-identity-row \
             documentation-product-row\"\
             >{project_brand}{primary_navigation}{product_controls}</div>"
        )
    } else if header.navigation.documentation {
        format!(
            "<div class=\"project-identity-row documentation-product-row\"><a \
             class=\"documentation-title\" \
             href=\"{prefix}\">{}</a>{primary_navigation}{product_controls}</\
             div>",
            esc(project_title)
        )
    } else {
        primary_navigation
    };
    let footer_navigation = header
        .navigation
        .html("footer", active, prefix)
        .unwrap_or_default();
    let outline_in_sidebar =
        header.chrome.outline_in_sidebar() && !page_outline.is_empty();
    let integrated_outline = if outline_in_sidebar && !page.hidden.outline() {
        page_outline.replacen(
            "class=\"page-outline\"",
            "class=\"integrated-outline\"",
            1,
        )
    } else {
        String::new()
    };
    let sidebar_class = if header.chrome.sidebar_sections() {
        "site-navigation sidebar-sections"
    } else {
        "site-navigation"
    };
    let navigation_panel = if without_sidebar ||
        page.hidden.navigation() ||
        (project_page && header.navigation.documentation)
    {
        String::new()
    } else if let Some(sidebar) = explicit_sidebar {
        format!(
            "<aside class=\"{sidebar_class}\" data-project=\"{}\" \
             data-target=\"{}\"><details open><summary>Browse \
             documentation</summary>{sidebar}{integrated_outline}</details></\
             aside>",
            esc(plan.project.as_str()),
            esc(plan.target.as_str())
        )
    } else if project_page && !header.chrome.project_navigation() {
        String::new()
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
    let search_dialog = if header.chrome.search() {
        super::search::dialog(prefix)
    } else {
        String::new()
    };
    let body = if project_page {
        super::project::sections(
            &format!("{heading}{}", page.body),
            page.panels,
            page.presentation,
        )?
    } else {
        page.body.into()
    };
    let heading = if project_page { "" } else { heading };
    let (root_label, root_route) =
        page.breadcrumb_root.unwrap_or(("Docs", prefix));
    let trail = if project_page || !header.chrome.breadcrumbs() {
        String::new()
    } else {
        format!(
            "<div class=\"breadcrumbs\"><a href=\"{}\">{}</a><span \
             aria-hidden=\"true\">/</span>{breadcrumbs}<span>{}</span></div>",
            esc(root_route),
            esc(root_label),
            esc(page.title)
        )
    };
    let neighbors = if project_page || !header.chrome.page_neighbors() {
        String::new()
    } else {
        neighbors
    };
    let page_outline = if project_page ||
        page.hidden.outline() ||
        header.chrome.outline_in_sidebar()
    {
        ""
    } else {
        page_outline
    };
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
    let band_attribute = if header.status_band.shown() {
        " data-status-band"
    } else {
        ""
    };
    let status_band = header.status_band.html();
    Ok(format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{page_title} · {project_title}</title><meta name="generator" content="GraphArch"><meta name="grapharch-revision" content="{revision}"><meta name="grapharch-target" content="{target}"><meta name="grapharch-page" content="{id}"><meta name="grapharch-fragment-count" content="{fragment_count}">{prepaint}{extra_head}<link rel="stylesheet" href="{prefix}style.css"><link rel="icon" href="data:,"></head>
<body{body_attribute}{layout_attribute}{band_attribute}><a class="skip-link" href="#main-content">Skip to content</a>{status_band}
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
        body = with_facts(&body, &facts),
    ))
}

/// Place the project facts after the page's first heading, else first.
fn with_facts(body: &str, facts: &str) -> String {
    if facts.is_empty() {
        return body.to_owned();
    }
    match body.find("</h1>") {
        Some(at) => format!("{}{facts}{}", &body[..at + 5], &body[at + 5..]),
        None => format!("{facts}{body}"),
    }
}
