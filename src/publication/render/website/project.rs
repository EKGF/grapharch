//! Semantic project landing-page sections derived from ordinary Markdown
//! headings.
use scraper::{ElementRef, Html};

/// Group the already rendered Markdown; no authored HTML or source-file access.
pub(super) fn sections(body: &str) -> String {
    let fragment = Html::parse_fragment(body);
    let mut output = String::from(
        "<div class=\"project-hero\"><div class=\"project-container\">",
    );
    let mut section = false;
    let mut card = false;
    for node in fragment.root_element().children() {
        let Some(element) = ElementRef::wrap(node) else {
            continue
        };
        match element.value().name() {
            "h2" => {
                if card {
                    output.push_str("</div>");
                    card = false;
                }
                output.push_str(
                    if section {
                        "</div></section>"
                    } else {
                        "</div></div>"
                    },
                );
                output.push_str(
                    "<section class=\"project-section\"><div \
                     class=\"project-container\">",
                );
                section = true;
            },
            "h3" if section => {
                if card {
                    output.push_str("</div>");
                }
                output.push_str("<div class=\"project-card\">");
                card = true;
            },
            _ => {},
        }
        let html = element.html();
        if !section && action_paragraph(element) {
            output.push_str(&html.replacen(
                "<p>",
                "<p class=\"project-actions\">",
                1,
            ));
        } else {
            output.push_str(&html);
        }
    }
    if card {
        output.push_str("</div>");
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
