//! Semantic project landing-page sections derived from ordinary Markdown
//! headings.
use {
    super::super::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    scraper::{ElementRef, Html},
};

mod presentation;
pub(super) use presentation::Presentation;

pub(super) async fn panels(
    plan: &Plan,
    page: &NamedNode,
) -> Result<Vec<String>> {
    // bounded: one page can select at most 32 section panels.
    let mut panels = Vec::new();
    let mut rows = plan.source.objects(page, "panelSection").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            panels.len() < 32,
            "page exceeds 32 panel sections"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("panel section must be a heading anchor literal")
        };
        let anchor = value.value();
        ensure!(
            !anchor.is_empty() &&
                anchor.len() <= 128 &&
                anchor
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_')),
            "invalid panel section anchor {anchor}"
        );
        panels.push(anchor.to_owned());
    }
    Ok(panels)
}

/// Group the already rendered Markdown; no authored HTML or source-file access.
pub(super) fn sections(
    body: &str,
    panels: &[String],
    presentation: &Presentation,
) -> String {
    let fragment = Html::parse_fragment(body);
    let mut output = format!(
        "<div class=\"project-hero project-hero-{}\"><div \
         class=\"project-container\">",
        if presentation.landing {
            "landing"
        } else {
            "introduction"
        }
    );
    let mut section = false;
    let mut card = false;
    let mut resource = false;
    for node in fragment.root_element().children() {
        let Some(element) = ElementRef::wrap(node) else {
            continue
        };
        match element.value().name() {
            "h2" => {
                if card {
                    output.push_str(
                        if resource { "</div></div>" } else { "</div>" },
                    );
                    card = false;
                }
                output.push_str(
                    if section {
                        "</div></section>"
                    } else {
                        "</div></div>"
                    },
                );
                let panel = element
                    .value()
                    .attr("id")
                    .is_some_and(|id| panels.iter().any(|anchor| anchor == id));
                output.push_str(
                    if panel {
                        "<section class=\"project-section \
                         project-section-panel\"><div \
                         class=\"project-container\">"
                    } else {
                        "<section class=\"project-section\"><div \
                         class=\"project-container\">"
                    },
                );
                if element.value().attr("id").is_some_and(|id| {
                    presentation.features.iter().any(|anchor| anchor == id)
                }) {
                    let marker = output
                        .rfind("<section class=\"project-section")
                        .unwrap();
                    output.insert_str(
                        marker + "<section class=\"project-section".len(),
                        " project-section-features",
                    );
                }
                resource = element.value().attr("id").is_some_and(|id| {
                    presentation.resources.iter().any(|anchor| anchor == id)
                });
                if resource {
                    let marker = output
                        .rfind("<section class=\"project-section")
                        .unwrap();
                    output.insert_str(
                        marker + "<section class=\"project-section".len(),
                        " project-section-resources",
                    );
                }
                section = true;
            },
            "h3" if section => {
                if card {
                    output.push_str(
                        if resource { "</div></div>" } else { "</div>" },
                    );
                }
                output.push_str("<div class=\"project-card\">");
                if let Some((name, tone)) = element
                    .value()
                    .attr("id")
                    .and_then(|id| presentation.icons.get(id))
                {
                    output.push_str(&format!(
                        "<span class=\"project-card-icon \
                         project-card-icon-{tone}\" \
                         aria-hidden=\"true\">{}</span>",
                        presentation::icon_svg(name).expect("validated icon")
                    ));
                }
                if resource {
                    output.push_str("<div class=\"project-card-content\">");
                }
                card = true;
            },
            _ => {},
        }
        let html = element.html();
        if section &&
            element.value().name() == "h2" &&
            element
                .value()
                .attr("id")
                .is_some_and(|id| presentation.icons.contains_key(id))
        {
            let (name, tone) =
                &presentation.icons[element.value().attr("id").unwrap()];
            output.push_str(&format!(
                "<div class=\"project-section-heading\"><span \
                 class=\"project-card-icon project-card-icon-{tone}\" \
                 aria-hidden=\"true\">{}</span>{html}</div>",
                presentation::icon_svg(name).expect("validated icon")
            ));
        } else if !section && action_paragraph(element) {
            output.push_str(&html.replacen(
                "<p>",
                "<p class=\"project-actions\">",
                1,
            ));
        } else if card && action_paragraph(element) {
            let mut actions =
                html.replacen("<p>", "<p class=\"project-card-action\">", 1);
            for link in element.child_elements() {
                let icon = if link.value().attr("href").is_some_and(|href| {
                    href.starts_with("https://") || href.starts_with("http://")
                }) {
                    include_str!("project/icons/external-link.svg")
                } else {
                    include_str!("project/icons/arrow-right.svg")
                };
                actions = actions.replace(
                    &link.html(),
                    &link.html().replace("</a>", &format!("{icon}</a>")),
                );
            }
            output.push_str(&actions);
        } else if section && action_paragraph(element) {
            output.push_str(&html.replacen(
                "<p>",
                "<p class=\"project-section-actions\">",
                1,
            ));
        } else {
            output.push_str(&html);
        }
    }
    if card {
        output.push_str(if resource { "</div></div>" } else { "</div>" });
    }
    output.push_str(
        if section {
            "</div></section>"
        } else {
            "</div></div>"
        },
    );
    output
}

// A paragraph consisting solely of links is a call-to-action row. An ordinary
// introductory sentence containing an inline link remains normal prose.
fn action_paragraph(element: ElementRef<'_>) -> bool {
    element.value().name() == "p" &&
        element
            .children()
            .any(|node| ElementRef::wrap(node).is_some()) &&
        element.children().all(|node| {
            if let Some(child) = ElementRef::wrap(node) {
                child.value().name() == "a"
            } else {
                node.value()
                    .as_text()
                    .is_some_and(|text| text.trim().is_empty())
            }
        })
}
