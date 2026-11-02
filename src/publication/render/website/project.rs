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
        output.push_str(&element.html());
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
