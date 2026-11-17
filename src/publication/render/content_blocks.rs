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
    // bounded: one entry per open table, at most the 16-container nesting.
    tables:   Vec<TableCells>,
}
/// Cell progress of one open table, which closes only on a complete row.
struct TableCells {
    columns: usize,
    cells:   usize,
}
impl TableCells {
    fn new(columns: usize) -> Self { Self { columns, cells: 0 } }

    /// Count a cell, returning whether it opens and whether it closes a row.
    fn step(&mut self) -> (bool, bool) {
        let column = self.cells % self.columns;
        self.cells += 1;
        (column == 0, column + 1 == self.columns)
    }

    fn finish(self) -> Result<()> {
        anyhow::ensure!(
            self.cells.is_multiple_of(self.columns),
            "table ends inside a row"
        );
        Ok(())
    }
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
                Boundary::Figure { .. } | Boundary::TableFigure { .. } => {
                    "</figure>\n".into()
                },
                Boundary::Caption => "</figcaption>\n".into(),
                Boundary::Table { .. } => {
                    self.close_table()?;
                    "</tbody></table></div>\n".into()
                },
                Boundary::Cell => {
                    let closes = self.current_row_closes()?;
                    if closes { "</td></tr>\n" } else { "</td>\n" }.into()
                },
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
                Boundary::Figure { max_width_px } => {
                    format!(
                        "<figure class=\"ga-figure\"{}>\n",
                        max_width_px
                            .map(|width| {
                                format!(" style=\"--figure-width:{width}px\"")
                            })
                            .unwrap_or_default()
                    )
                },
                Boundary::Caption => "<figcaption>\n".into(),
                Boundary::TableFigure { caption, label } => {
                    format!(
                        "<figure class=\"ga-table-figure\" \
                         id=\"{}\"><figcaption>{}</figcaption>\n",
                        escape(&label),
                        escape(&caption)
                    )
                },
                Boundary::Table { columns } => {
                    self.tables.push(TableCells::new(columns.len()));
                    format!(
                        "<div class=\"table-scroll\" role=\"region\" \
                         aria-label=\"Data table\" tabindex=\"0\"><table \
                         class=\"ga-reference-table\"><colgroup>{}</\
                         colgroup><thead><tr>{}</tr></thead><tbody>\n",
                        columns
                            .iter()
                            .map(|column| {
                                format!(
                                    "<col style=\"width:{}%\">",
                                    column.percent
                                )
                            })
                            .collect::<String>(),
                        columns
                            .iter()
                            .map(|column| {
                                format!(
                                    "<th scope=\"col\">{}</th>",
                                    escape(&column.title)
                                )
                            })
                            .collect::<String>()
                    )
                },
                Boundary::Cell => {
                    let (opens, _) = self.step_cell()?;
                    if opens { "<tr><td>\n" } else { "<td>\n" }.into()
                },
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
                Boundary::Figure { .. } => "]])\n".into(),
                Boundary::Caption => "]]\n".into(),
                Boundary::TableFigure { label, .. } => {
                    format!("]#label({})]\n", quote(&label))
                },
                Boundary::Table { .. } => {
                    self.close_table()?;
                    ")\n".into()
                },
                Boundary::Cell => {
                    self.current_row_closes()?;
                    "],\n".into()
                },
                Boundary::Admonition { .. } => "]\n".into(),
                _ => "\n".into(),
            }
        } else {
            match value {
                Boundary::Figure { max_width_px } => {
                    format!(
                        "#layout(size => block(width: {})[#figure(kind: \
                         image, numbering: none, supplement: none)[\n",
                        max_width_px
                            .map(|width| {
                                format!(
                                    "calc.min(size.width, {}pt)",
                                    f64::from(width) * 0.75
                                )
                            })
                            .unwrap_or_else(|| "size.width".into())
                    )
                },
                Boundary::Caption => {
                    "#block(above: 6pt)[#text(size: 9pt)[\n".into()
                },
                // A figure holding tables may span pages. Its body keeps the
                // document's left alignment, and its number is the theme's
                // table numbering.
                Boundary::TableFigure { caption, .. } => {
                    format!(
                        "#block(width: 100%, above: 12pt, below: 12pt)[#show \
                         figure: set block(breakable: true)\n#figure(kind: \
                         table, supplement: [Table], numbering: \
                         ga-table-numbering, caption: [#text({})])[#set \
                         align(left)\n",
                        quote(&caption)
                    )
                },
                Boundary::Table { columns } => {
                    self.tables.push(TableCells::new(columns.len()));
                    format!(
                        "#ga-table(columns: ({},), align: ({},), header-row: \
                         0, cell-spacing: 0.8em, table.header({}), ",
                        columns
                            .iter()
                            .map(|column| format!("{}%", column.percent))
                            .collect::<Vec<_>>()
                            .join(", "),
                        vec!["left"; columns.len()].join(", "),
                        columns
                            .iter()
                            .map(|column| {
                                format!(
                                    "[#strong[#text({})]]",
                                    quote(&column.title)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
                Boundary::Cell => {
                    self.step_cell()?;
                    "[".into()
                },
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

    /// Whether the content being converted is inside a table cell.
    pub fn in_cell(&self) -> bool { self.stack.in_cell() }

    fn step_cell(&mut self) -> Result<(bool, bool)> {
        Ok(self
            .tables
            .last_mut()
            .ok_or_else(|| anyhow::anyhow!("table cell outside a table"))?
            .step())
    }

    fn current_row_closes(&self) -> Result<bool> {
        let table = self
            .tables
            .last()
            .ok_or_else(|| anyhow::anyhow!("table cell outside a table"))?;
        Ok(table.cells.is_multiple_of(table.columns))
    }

    fn close_table(&mut self) -> Result<()> {
        self.tables
            .pop()
            .ok_or_else(|| anyhow::anyhow!("unbalanced table"))?
            .finish()
    }

    pub fn finish(self) -> Result<()> {
        anyhow::ensure!(self.tables.is_empty(), "unclosed table");
        self.stack.finish()
    }
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

#[cfg(test)]
#[path = "content_blocks_tests.rs"]
mod tests;
