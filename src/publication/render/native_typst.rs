//! Declarative native HTML to Typst. Every source string is quoted as data.
use {
    super::{markup::quote, normative::Conformance},
    crate::publication::{diagnostics::Diagnostic, native_html},
    anyhow::{Result, ensure},
    scraper::{ElementRef, Html, Selector},
};

/// Conversion context: diagnostics mark unresolved references and the
/// target's conformance language styles native normative keywords.
pub(super) struct NativeTypst<'a> {
    diagnostics: &'a [Diagnostic],
    conformance: Conformance,
}

impl<'a> NativeTypst<'a> {
    pub(super) fn new(
        diagnostics: &'a [Diagnostic],
        conformance: Conformance,
    ) -> Self {
        Self { diagnostics, conformance }
    }

    pub(super) fn convert(&self, body: &str) -> Result<String> {
        ensure!(
            body.len() <= 1024 * 1024,
            "native Typst fragment exceeds 1 MiB"
        );
        let document = Html::parse_fragment(body);
        self.children(document.root_element())
    }

    fn children(&self, element: ElementRef<'_>) -> Result<String> {
        let mut out = String::new();
        let inline = matches!(
            element.value().name(),
            "p" | "a" |
                "span" |
                "strong" |
                "b" |
                "em" |
                "i" |
                "code" |
                "dfn" |
                "dt" |
                "dd" |
                "li" |
                "td" |
                "th" |
                "abbr" |
                "q" |
                "cite" |
                "summary"
        );
        for child in element.children() {
            if let Some(text) = child.value().as_text() {
                let normalized = if text.trim().is_empty() {
                    if inline {
                        " ".to_owned()
                    } else {
                        String::new()
                    }
                } else {
                    let mut value =
                        text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if text.starts_with(char::is_whitespace) {
                        value.insert(0, ' ');
                    }
                    if text.ends_with(char::is_whitespace) {
                        value.push(' ');
                    }
                    value
                };
                if !normalized.is_empty() {
                    out.push_str(&format!("#text({})", quote(&normalized)));
                }
            } else if let Some(child) = ElementRef::wrap(child) {
                out.push_str(&self.emit(child)?);
            }
        }
        Ok(out)
    }

    fn emit(&self, element: ElementRef<'_>) -> Result<String> {
        let tag = element.value().name();
        let mut out = match tag {
            "pre" => {
                let text = element.text().collect::<String>();
                let language = element
                    .value()
                    .classes()
                    .chain(
                        element
                            .children()
                            .filter_map(ElementRef::wrap)
                            .flat_map(|child| child.value().classes()),
                    )
                    .find_map(|class| class.strip_prefix("language-"))
                    .unwrap_or("");
                super::code::typst(&text, language)?
            },
            "table" => self.table(element)?,
            "ul" | "ol" => {
                let function = if tag == "ul" { "list" } else { "enum" };
                let mut items = String::new();
                for child in element.children().filter_map(ElementRef::wrap) {
                    ensure!(
                        child.value().name() == "li",
                        "unsupported native list child"
                    );
                    items.push_str(&format!(
                        "[{}{}],",
                        self.children(child)?,
                        label(child)
                    ));
                }
                let start = if tag == "ol" {
                    format!("start: {}, ", number(element, "start", 1)?)
                } else {
                    String::new()
                };
                format!("#{function}({start}{items})\n")
            },
            "h2" | "h3" | "h4" | "h5" | "h6" => {
                let title = native_html::title(element);
                let numbered = element.value().attr("data-grapharch-numbered") ==
                    Some("true");
                let toc =
                    element.value().attr("data-grapharch-toc") == Some("true");
                if numbered || toc {
                    let level = tag.as_bytes()[1] - b'1';
                    let numbering =
                        if numbered { "" } else { ", numbering: none" };
                    format!(
                        "#heading(level: {level}{numbering})[#text({})]",
                        quote(&title)
                    )
                } else {
                    format!(
                        "#block(above: 8pt, below: 5pt)[#strong[#text({})]]",
                        quote(&title)
                    )
                }
            },
            "a" => {
                let content = self.children(element)?;
                if let Some(href) = element.value().attr("href") {
                    if self
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.reference == href)
                    {
                        format!("#text(fill: rgb(\"#a01515\"))[{content}]")
                    } else {
                        let destination =
                            if let Some(id) = href.strip_prefix('#') {
                                format!("label({})", quote(id))
                            } else {
                                quote(href)
                            };
                        format!("#link({destination})[{content}]")
                    }
                } else {
                    content
                }
            },
            "strong" | "b" | "dfn" => {
                format!("#strong[{}]", self.children(element)?)
            },
            "em" if element
                .value()
                .classes()
                .any(|class| class == "rfc2119") =>
            {
                self.conformance.native(&self.children(element)?)
            },
            "em" | "i" | "cite" | "var" => {
                format!("#emph[{}]", self.children(element)?)
            },
            "code" | "samp" | "kbd" => {
                format!(
                    "#text(font: \"DejaVu Sans Mono\", size: 8pt)[{}]",
                    self.children(element)?
                )
            },
            "sub" | "sup" => format!("#{tag}[{}]", self.children(element)?),
            "del" => format!("#strike[{}]", self.children(element)?),
            "p" if element
                .value()
                .classes()
                .any(|class| class == "section-title") =>
            {
                format!(
                    "#block(above: 14pt, below: 6pt, sticky: \
                     true)[#text(fill: ga-accent, weight: \"semibold\")[{}]]\n",
                    self.children(element)?
                )
            },
            "p" => {
                format!(
                    "#block(above: 5pt, below: 5pt, breakable: true)[{}]\n",
                    self.children(element)?
                )
            },
            "dt" | "summary" => {
                format!(
                    "#v(6pt, weak: false)\n#block(sticky: true)[#strong[{}]]\n",
                    self.children(element)?
                )
            },
            "dd" => {
                format!(
                    "#v(3pt, weak: false)\n#block(inset: (left: 12pt), above: \
                     1pt, below: 3pt, breakable: true)[{}]\n\n",
                    self.children(element)?
                )
            },
            "blockquote" => {
                format!("#quote(block: true)[{}]", self.children(element)?)
            },
            "br" => "#linebreak()".into(),
            "hr" => "#line(length: 100%)".into(),
            "wbr" => String::new(),
            "section" | "span" | "div" | "dl" | "details" | "figure" |
            "figcaption" | "aside" | "abbr" | "time" | "bdi" | "mark" |
            "q" | "ins" => self.children(element)?,
            other => anyhow::bail!("unsupported native Typst element {other}"),
        };
        out.push_str(&label(element));
        Ok(out)
    }

    fn table(&self, element: ElementRef<'_>) -> Result<String> {
        let selector = Selector::parse("tr").unwrap();
        let rows: Vec<_> = element.select(&selector).collect();
        ensure!(
            rows.len() <= 10_000,
            "native table exceeds row budget"
        );
        let mut columns = 0;
        for row in &rows {
            let mut count = 0;
            for cell in row.children().filter_map(ElementRef::wrap) {
                count += number(cell, "colspan", 1)?;
            }
            columns = columns.max(count);
        }
        ensure!(
            (1..=32).contains(&columns),
            "native table column count must be 1..32"
        );
        let widths = if columns == 4 &&
            element.value().classes().any(|class| class == "grammar")
        {
            "(auto, auto, auto, 1fr)".to_owned()
        } else {
            columns.to_string()
        };
        let mut cells = String::new();
        let mut in_header = false;
        let mut has_header = false;
        for (row_index, row) in rows.into_iter().enumerate() {
            let header = row
                .children()
                .filter_map(ElementRef::wrap)
                .all(|cell| cell.value().name() == "th") &&
                (row_index == 0 || in_header);
            if header && !in_header {
                cells.push_str("table.header(");
                in_header = true;
                has_header = true;
            } else if !header && in_header {
                cells.push_str("),");
                in_header = false;
            }
            for (index, cell) in
                row.children().filter_map(ElementRef::wrap).enumerate()
            {
                ensure!(
                    matches!(cell.value().name(), "td" | "th"),
                    "unsupported native table child"
                );
                let mut body = self.children(cell)?;
                if cell.value().name() == "th" {
                    body = format!("#strong[{body}]");
                }
                body.push_str(&label(cell));
                if index == 0 {
                    body.push_str(&label(row));
                }
                cells.push_str(&format!(
                    "table.cell(colspan: {}, rowspan: {})[{body}],",
                    number(cell, "colspan", 1)?,
                    number(cell, "rowspan", 1)?
                ));
            }
        }
        if in_header {
            cells.push_str("),");
        }
        Ok(format!(
            "#ga-table(columns: {widths}, has-header: {has_header}, {cells})\n"
        ))
    }
}

fn label(element: ElementRef<'_>) -> String {
    element
        .value()
        .attr("id")
        .map(|id| format!("#metadata({})#label({})", quote(id), quote(id)))
        .unwrap_or_default()
}
fn number(element: ElementRef<'_>, name: &str, default: u32) -> Result<u32> {
    let value = element
        .value()
        .attr(name)
        .map(str::parse::<u32>)
        .transpose()?
        .unwrap_or(default);
    ensure!(
        (1..=10_000).contains(&value),
        "native {name} exceeds supported range"
    );
    Ok(value)
}
