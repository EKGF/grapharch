//! Declarative native HTML to Typst. Every source string is quoted as data.
use {
    super::markup::quote,
    crate::publication::{diagnostics::Diagnostic, native_html},
    anyhow::{Result, ensure},
    scraper::{ElementRef, Html, Selector},
};

pub(super) fn convert(
    body: &str,
    diagnostics: &[Diagnostic],
) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "native Typst fragment exceeds 1 MiB"
    );
    let document = Html::parse_fragment(body);
    children(document.root_element(), diagnostics)
}

fn children(
    element: ElementRef<'_>,
    diagnostics: &[Diagnostic],
) -> Result<String> {
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
            out.push_str(&emit(child, diagnostics)?);
        }
    }
    Ok(out)
}

fn emit(element: ElementRef<'_>, diagnostics: &[Diagnostic]) -> Result<String> {
    let tag = element.value().name();
    let mut out = match tag {
        "pre" => {
            let text = element.text().collect::<String>();
            let mut code = String::new();
            for line in text.lines() {
                code.push_str(&format!("#text({})#linebreak()\n", quote(line)));
            }
            format!(
                "#block(fill: rgb(\"#f5f5f5\"), inset: 6pt, breakable: \
                 true)[#set text(font: \"DejaVu Sans Mono\", size: 7.5pt, \
                 hyphenate: false)\n{code}]\n"
            )
        },
        "table" => table(element, diagnostics)?,
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
                    children(child, diagnostics)?,
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
            let numbered =
                element.value().attr("data-grapharch-numbered") == Some("true");
            let toc =
                element.value().attr("data-grapharch-toc") == Some("true");
            if numbered || toc {
                let level = tag.as_bytes()[1] - b'1';
                let numbering = if numbered { "" } else { ", numbering: none" };
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
            let content = children(element, diagnostics)?;
            if let Some(href) = element.value().attr("href") {
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.reference == href)
                {
                    format!("#text(fill: rgb(\"#a01515\"))[{content}]")
                } else {
                    let destination = if let Some(id) = href.strip_prefix('#') {
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
            format!("#strong[{}]", children(element, diagnostics)?)
        },
        "em" | "i" | "cite" | "var" => {
            format!("#emph[{}]", children(element, diagnostics)?)
        },
        "code" | "samp" | "kbd" => {
            format!(
                "#text(font: \"DejaVu Sans Mono\", size: 8pt)[{}]",
                children(element, diagnostics)?
            )
        },
        "sub" | "sup" => format!("#{tag}[{}]", children(element, diagnostics)?),
        "del" => format!("#strike[{}]", children(element, diagnostics)?),
        "p" => {
            format!(
                "#block(above: 5pt, below: 5pt, breakable: true)[{}]\n",
                children(element, diagnostics)?
            )
        },
        "dt" | "summary" => {
            format!(
                "#block(above: 5pt, below: 2pt)[#strong[{}]]",
                children(element, diagnostics)?
            )
        },
        "dd" => {
            format!(
                "#block(inset: (left: 12pt), above: 1pt, below: 3pt, \
                 breakable: true)[{}]",
                children(element, diagnostics)?
            )
        },
        "blockquote" => {
            format!(
                "#quote(block: true)[{}]",
                children(element, diagnostics)?
            )
        },
        "br" => "#linebreak()".into(),
        "hr" => "#line(length: 100%)".into(),
        "wbr" => String::new(),
        "section" | "span" | "div" | "dl" | "details" | "figure" |
        "figcaption" | "aside" | "abbr" | "time" | "bdi" | "mark" | "q" |
        "ins" => children(element, diagnostics)?,
        other => anyhow::bail!("unsupported native Typst element {other}"),
    };
    out.push_str(&label(element));
    Ok(out)
}

fn table(
    element: ElementRef<'_>,
    diagnostics: &[Diagnostic],
) -> Result<String> {
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
    for row in rows {
        for (index, cell) in
            row.children().filter_map(ElementRef::wrap).enumerate()
        {
            ensure!(
                matches!(cell.value().name(), "td" | "th"),
                "unsupported native table child"
            );
            let mut body = children(cell, diagnostics)?;
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
    Ok(format!(
        "#block(above: 6pt, below: 6pt, breakable: true)[#set text(size: \
         8.5pt)\n#table(columns: {widths}, inset: 3pt, stroke: 0.3pt + \
         rgb(\"#cbd5e1\"), {cells})]\n"
    ))
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
