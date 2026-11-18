//! Group already rendered Markdown into the project page's hero, sections and
//! cards; no authored HTML or source-file access.
//!
//! A level-two heading starts a section and a level-three heading a card. A
//! thematic break ends the current card, so closing prose belongs to the
//! section again. Cards directly after the introduction form an untitled
//! section; naming the page title's anchor as a resource section presents
//! them as a resource list.
use {
    super::{presentation::Presentation, timeline},
    anyhow::Result,
    scraper::{ElementRef, Html},
};

/// Group the rendered Markdown `body` of one project page.
pub(in crate::publication::render::website) fn sections(
    body: &str,
    panels: &[String],
    presentation: &Presentation,
) -> Result<String> {
    let mut builder = Builder {
        panels,
        presentation,
        output: format!(
            "<div class=\"project-hero project-hero-{}\"><div \
             class=\"project-container\">",
            if presentation.landing {
                "landing"
            } else {
                "introduction"
            }
        ),
        title: None,
        section: None,
        card: None,
    };
    let fragment = Html::parse_fragment(body);
    for node in fragment.root_element().children() {
        if let Some(element) = ElementRef::wrap(node) {
            builder.element(element)?;
        }
    }
    Ok(builder.finish())
}

/// How the current section presents its cards.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Grid,
    Resources,
    Timeline,
    Terms,
}

/// The open card; a linked resource card closes with a trailing arrow.
struct Card {
    resource: bool,
    arrow:    bool,
    /// A term row; `definition` once its definition column has opened.
    term:     Option<TermRow>,
}

/// The open row of a term list.
#[derive(Clone, Copy)]
struct TermRow {
    definition: bool,
    /// The trailing icon of a term whose heading is one link.
    icon:       Option<&'static str>,
}

struct Builder<'a> {
    panels:       &'a [String],
    presentation: &'a Presentation,
    output:       String,
    /// The page title's anchor, once seen.
    title:        Option<String>,
    section:      Option<Kind>,
    card:         Option<Card>,
}

impl Builder<'_> {
    fn element(&mut self, element: ElementRef) -> Result<()> {
        let id = element.value().attr("id");
        match element.value().name() {
            "h1" => self.title = id.map(str::to_owned),
            "h2" => {
                self.close_card();
                self.open_section(id);
                if let Some(style) =
                    id.and_then(|id| self.presentation.icons.get(id))
                {
                    self.output.push_str(&format!(
                        "<div class=\"project-section-heading\">{}{}</div>",
                        style.marker.html(),
                        element.html()
                    ));
                    return Ok(());
                }
            },
            "h3" => {
                self.open_card(element);
                return Ok(());
            },
            "hr" if self.section.is_some() => {
                self.close_card();
                return Ok(());
            },
            "ol" if self.section == Some(Kind::Timeline) &&
                self.card.is_none() =>
            {
                self.output.push_str(&timeline::html(
                    element,
                    &self.presentation.statuses,
                )?);
                return Ok(());
            },
            _ => {},
        }
        self.content(element);
        Ok(())
    }

    fn content(&mut self, element: ElementRef) {
        let html = element.html();
        if let Some(row) =
            self.card.as_mut().and_then(|card| card.term.as_mut())
        {
            if !row.definition && strong_paragraph(element) {
                self.output.push_str(&html.replacen(
                    "<p>",
                    "<p class=\"project-term-expansion\">",
                    1,
                ));
                return;
            }
            if !row.definition {
                row.definition = true;
                self.output
                    .push_str("</div><div class=\"project-term-definition\">");
            }
            self.output.push_str(&html);
            return;
        }
        if self.section.is_none() && action_paragraph(element) {
            self.output.push_str(&html.replacen(
                "<p>",
                "<p class=\"project-actions\">",
                1,
            ));
        } else if self.card.is_some() && action_paragraph(element) {
            self.output
                .push_str(&linked_actions(element, "project-card-action"));
        } else if self.card.is_some() && emphasis_paragraph(element) {
            self.output.push_str(&html.replacen(
                "<p>",
                "<p class=\"project-card-meta\">",
                1,
            ));
        } else if self.section.is_some() && action_paragraph(element) {
            self.output.push_str(&linked_actions(
                element,
                "project-section-actions",
            ));
        } else {
            self.output.push_str(&html);
        }
    }

    fn open_section(&mut self, id: Option<&str>) {
        self.output.push_str(
            if self.section.is_some() {
                "</div></section>"
            } else {
                "</div></div>"
            },
        );
        let selected = |anchors: &[String]| {
            id.is_some_and(|id| anchors.iter().any(|anchor| anchor == id))
        };
        let kind = if selected(&self.presentation.resources) {
            Kind::Resources
        } else if selected(&self.presentation.timelines) {
            Kind::Timeline
        } else if selected(&self.presentation.terms) {
            Kind::Terms
        } else {
            Kind::Grid
        };
        let mut classes = String::from("project-section");
        if selected(self.panels) {
            classes.push_str(" project-section-panel");
        }
        if selected(&self.presentation.features) {
            classes.push_str(" project-section-features");
        }
        if selected(&self.presentation.callouts) {
            classes.push_str(" project-section-callout");
        }
        if selected(&self.presentation.compacts) {
            classes.push_str(" project-section-compact");
        }
        classes.push_str(match kind {
            Kind::Resources => " project-section-resources",
            Kind::Timeline => " project-section-timeline",
            Kind::Terms => " project-section-terms",
            Kind::Grid => "",
        });
        self.output.push_str(&format!(
            "<section class=\"{classes}\"><div class=\"project-container\">"
        ));
        self.section = Some(kind);
    }

    fn open_card(&mut self, heading: ElementRef) {
        self.close_card();
        if self.section.is_none() {
            let title = self.title.clone();
            self.open_section(title.as_deref());
        }
        let style = heading
            .value()
            .attr("id")
            .and_then(|id| self.presentation.icons.get(id));
        if self.section == Some(Kind::Terms) {
            let tone =
                style.map_or("primary", |style| style.marker.tone.class());
            self.output.push_str(&format!(
                "<div class=\"project-term project-term-{tone}\"><div \
                 class=\"project-term-name\">{}",
                heading.html()
            ));
            self.card = Some(Card {
                resource: false,
                arrow:    false,
                term:     Some(TermRow {
                    definition: false,
                    icon:       title_link(heading).then(|| link_icon(heading)),
                }),
            });
            return;
        }
        let linked = title_link(heading);
        let resource = self.section == Some(Kind::Resources);
        self.output.push_str(
            if linked {
                "<div class=\"project-card project-card-linked\">"
            } else {
                "<div class=\"project-card\">"
            },
        );
        if let Some(style) = style {
            self.output.push_str(&style.marker.html());
        }
        if resource {
            self.output.push_str("<div class=\"project-card-content\">");
        }
        match style.and_then(|style| style.badge_html()) {
            Some(badge) => {
                self.output.push_str(&format!(
                    "<div class=\"project-card-title\">{}{badge}</div>",
                    heading.html()
                ))
            },
            None => self.output.push_str(&heading.html()),
        }
        self.card =
            Some(Card { resource, arrow: resource && linked, term: None });
    }

    fn close_card(&mut self) {
        let Some(card) = self.card.take() else { return };
        if let Some(row) = card.term {
            self.output.push_str("</div>");
            if let Some(icon) = row.icon {
                self.output.push_str(&format!(
                    "<span class=\"project-term-arrow\" \
                     aria-hidden=\"true\">{icon}</span>"
                ));
            }
            self.output.push_str("</div>");
            return;
        }
        if card.resource {
            self.output.push_str("</div>");
        }
        if card.arrow {
            self.output.push_str(&format!(
                "<span class=\"project-card-arrow\" \
                 aria-hidden=\"true\">{}</span>",
                include_str!("icons/arrow-right.svg")
            ));
        }
        self.output.push_str("</div>");
    }

    fn finish(mut self) -> String {
        self.close_card();
        self.output.push_str(
            if self.section.is_some() {
                "</div></section>"
            } else {
                "</div></div>"
            },
        );
        self.output
    }
}

/// Whether a card heading is exactly one link, so that the whole card can
/// act as that link with the heading as its accessible name.
fn title_link(heading: ElementRef) -> bool {
    let mut children = heading.child_elements();
    match (children.next(), children.next()) {
        (Some(link), None) => {
            link.value().name() == "a" &&
                link.text().collect::<String>() ==
                    heading.text().collect::<String>()
        },
        _ => false,
    }
}

/// A call-to-action row in `class`, each link marked internal or external.
fn linked_actions(element: ElementRef, class: &str) -> String {
    let mut actions =
        element
            .html()
            .replacen("<p>", &format!("<p class=\"{class}\">"), 1);
    for link in element.child_elements() {
        let icon = link_icon(link);
        actions = actions.replace(
            &link.html(),
            &link.html().replace("</a>", &format!("{icon}</a>")),
        );
    }
    actions
}

/// The icon marking where the first link within `element` leads: away from
/// the site, or to another of its pages.
fn link_icon(element: ElementRef) -> &'static str {
    let link = if element.value().name() == "a" {
        Some(element)
    } else {
        element
            .child_elements()
            .find(|child| child.value().name() == "a")
    };
    if link
        .and_then(|link| link.value().attr("href"))
        .is_some_and(|href| {
            href.starts_with("https://") || href.starts_with("http://")
        })
    {
        include_str!("icons/external-link.svg")
    } else {
        include_str!("icons/arrow-right.svg")
    }
}

/// Whether a paragraph consists only of `element_name` children and
/// whitespace, with at least one such child.
fn only_children(element: ElementRef<'_>, element_name: &str) -> bool {
    element.value().name() == "p" &&
        element
            .children()
            .any(|node| ElementRef::wrap(node).is_some()) &&
        element.children().all(|node| {
            if let Some(child) = ElementRef::wrap(node) {
                child.value().name() == element_name
            } else {
                node.value()
                    .as_text()
                    .is_some_and(|text| text.trim().is_empty())
            }
        })
}

// A paragraph consisting solely of links is a call-to-action row. An ordinary
// introductory sentence containing an inline link remains normal prose.
fn action_paragraph(element: ElementRef<'_>) -> bool {
    only_children(element, "a")
}

// A wholly strong paragraph names a term's expansion.
fn strong_paragraph(element: ElementRef<'_>) -> bool {
    only_children(element, "strong")
}

// A wholly emphasized card paragraph is secondary metadata, such as a path.
fn emphasis_paragraph(element: ElementRef<'_>) -> bool {
    only_children(element, "em")
}
