//! Renderer projections of the shared semantic container model.
use {
    super::markup::quote,
    crate::publication::content_blocks::{Boundary, Stack},
    anyhow::Result,
};
#[derive(Default)]
pub(super) struct Containers {
    stack:    Stack,
    pub tabs: bool,
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn tone(category: &str) -> &str {
    match category {
        "tip" | "success" => "tip",
        "warning" => "warning",
        "danger" | "failure" | "error" | "caution" => "caution",
        "important" | "principle" => "important",
        _ => "note",
    }
}
impl Containers {
    pub fn html(&mut self, source: &str) -> Result<String> {
        let value = Boundary::parse(source)?;
        let end = matches!(value, Boundary::End);
        let value = self.stack.step(value)?;
        Ok(if end {
            match value {
                Boundary::CardGroup |
                Boundary::Alternatives |
                Boundary::Alternative { .. } => "</section>\n".into(),
                Boundary::Admonition { collapsible: true, .. } => {
                    "</details>\n".into()
                },
                _ => "</aside>\n".into(),
            }
        } else {
            match value {
                Boundary::CardGroup => {
                    "<section class=\"ga-card-group\">\n".into()
                },
                Boundary::Alternatives => {
                    self.tabs = true;
                    "<section class=\"ga-tabs\">\n".into()
                },
                Boundary::Alternative { title } => {
                    format!(
                        "<section class=\"ga-tab\" data-title=\"{}\"><p \
                         class=\"ga-tab-title\"><strong>{}</strong></p>\n",
                        escape(&title),
                        escape(&title)
                    )
                },
                Boundary::Admonition {
                    category,
                    title,
                    collapsible,
                    expanded,
                } => {
                    let class = format!(
                        "ga-admonition ga-admonition-{}",
                        tone(&category)
                    );
                    if collapsible {
                        format!(
                            "<details class=\"{class}\"{}><summary>{}</\
                             summary>\n",
                            if expanded { " open" } else { "" },
                            escape(
                                if title.is_empty() {
                                    &category
                                } else {
                                    &title
                                }
                            )
                        )
                    } else {
                        format!(
                            "<aside class=\"{class}\" aria-label=\"{}\">{}\n",
                            escape(&category),
                            if title.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "<p class=\"callout-title\"><strong>{}</\
                                     strong></p>",
                                    escape(&title)
                                )
                            }
                        )
                    }
                },
                Boundary::End => unreachable!(),
            }
        })
    }

    pub fn typst(&mut self, source: &str) -> Result<String> {
        let value = Boundary::parse(source)?;
        let end = matches!(value, Boundary::End);
        let value = self.stack.step(value)?;
        Ok(if end {
            match value {
                Boundary::Admonition { .. } => "]\n".into(),
                _ => "\n".into(),
            }
        } else {
            match value {
                Boundary::Alternatives | Boundary::CardGroup => String::new(),
                Boundary::Alternative { title } => {
                    format!(
                        "#block(sticky: true, above: 12pt, below: \
                         6pt)[#text(weight: \"bold\", {})]\n",
                        quote(&title)
                    )
                },
                Boundary::Admonition { category, title, .. } => {
                    format!(
                        "#ga-callout({}, kind: {})[\n",
                        quote(&title),
                        quote(tone(&category))
                    )
                },
                Boundary::End => unreachable!(),
            }
        })
    }

    pub fn finish(self) -> Result<()> { self.stack.finish() }
}
pub(super) const SCRIPT: &str = include_str!("content_blocks.js");

/// Apply only the outer context of the selected contiguous fragment range.
pub(super) async fn enclosing(
    plan: &super::model::Plan,
    first: &oxrdf::NamedNode,
    last: &oxrdf::NamedNode,
    body: &str,
) -> Result<String> {
    let mut result = String::new();
    for (fragment, predicate, is_prefix) in [
        (first, "contentPrefix", true),
        (last, "contentSuffix", false),
    ] {
        if !is_prefix {
            result.push_str(body);
        }
        match plan.source.optional(fragment, predicate).await? {
            None => {},
            Some(oxrdf::Term::Literal(value)) => {
                anyhow::ensure!(
                    value.value().len() <= 64 * 1024,
                    "fragment content context exceeds byte budget"
                );
                result.push_str(value.value());
            },
            _ => anyhow::bail!("fragment content context must be literal"),
        }
    }
    Ok(result)
}
