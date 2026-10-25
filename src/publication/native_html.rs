//! Bounded, declarative HTML fragments from published specifications.
//! Executable document machinery is never admitted into shared publication
//! content.
use {
    anyhow::{Result, ensure},
    scraper::{ElementRef, Html},
    sha2::{Digest, Sha256},
    std::collections::HashSet,
    url::Url,
};

pub(crate) struct Heading {
    pub depth:    usize,
    pub id:       String,
    pub text:     String,
    pub numbered: bool,
    pub toc:      bool,
}

pub(crate) struct Fragment {
    pub html:     String,
    pub headings: Vec<Heading>,
    pub ids:      HashSet<String>,
    pub links:    Vec<String>,
}

pub(crate) fn read(
    body: &str,
    base: Option<&Url>,
    number: &mut dyn FnMut(usize) -> Result<Option<String>>,
) -> Result<Fragment> {
    ensure!(
        body.len() <= 1024 * 1024,
        "HTML fragment exceeds 1 MiB"
    );
    let document = Html::parse_fragment(body);
    let mut result = Fragment {
        html:     String::new(),
        headings: Vec::new(),
        ids:      HashSet::new(),
        links:    Vec::new(),
    };
    children(document.root_element(), base, number, &mut result)?;
    Ok(result)
}

fn children(
    element: ElementRef<'_>,
    base: Option<&Url>,
    number: &mut dyn FnMut(usize) -> Result<Option<String>>,
    out: &mut Fragment,
) -> Result<()> {
    for node in element.children() {
        if let Some(text) = node.value().as_text() {
            out.html.push_str(&escape(text));
        } else if let Some(element) = ElementRef::wrap(node) {
            emit(element, base, number, out)?;
        }
    }
    Ok(())
}

fn emit(
    element: ElementRef<'_>,
    base: Option<&Url>,
    number: &mut dyn FnMut(usize) -> Result<Option<String>>,
    out: &mut Fragment,
) -> Result<()> {
    let name = element.value().name();
    ensure!(
        matches!(
            name,
            "aside" |
                "h2" |
                "cite" |
                "section" |
                "p" |
                "pre" |
                "code" |
                "a" |
                "dfn" |
                "abbr" |
                "em" |
                "strong" |
                "span" |
                "table" |
                "thead" |
                "tbody" |
                "tfoot" |
                "tr" |
                "td" |
                "th" |
                "ul" |
                "ol" |
                "li" |
                "dl" |
                "dt" |
                "dd" |
                "div" |
                "b" |
                "i" |
                "sub" |
                "sup" |
                "br" |
                "hr" |
                "var" |
                "samp" |
                "kbd" |
                "figure" |
                "figcaption" |
                "caption" |
                "colgroup" |
                "col" |
                "wbr" |
                "details" |
                "summary" |
                "time" |
                "bdi" |
                "mark" |
                "q" |
                "blockquote" |
                "ins" |
                "del" |
                "h3" |
                "h4" |
                "h5" |
                "h6"
        ),
        "unsupported published HTML element {name}"
    );
    let wrap_table = name == "table" &&
        !element
            .parent()
            .and_then(ElementRef::wrap)
            .is_some_and(|parent| {
                parent
                    .value()
                    .classes()
                    .any(|class| class == "table-scroll")
            });
    if wrap_table {
        out.html.push_str(
            "<div class=\"table-scroll\" role=\"region\" \
             aria-label=\"Specification table\" tabindex=\"0\">",
        );
    }
    if matches!(name, "div" | "aside" | "figure") &&
        element.value().classes().any(|class| class == "example") &&
        element.value().attr("id").is_none()
    {
        let id = example_id(element);
        ensure!(
            out.ids.insert(id.clone()),
            "duplicate native HTML example {id}"
        );
        out.html.push_str(&format!("<{name} id=\"{id}\""));
    } else {
        out.html.push('<');
        out.html.push_str(name);
    }

    for (attribute, value) in element.value().attrs() {
        ensure!(
            !attribute.starts_with("on"),
            "executable HTML event attributes are unsupported"
        );
        if attribute == "id" {
            ensure!(
                !value.is_empty() && value.len() <= 512,
                "invalid native HTML anchor"
            );
            ensure!(
                out.ids.insert(value.to_owned()),
                "duplicate native HTML anchor {value}"
            );
        }
        if matches!(
            attribute,
            "id" | "class" |
                "title" |
                "lang" |
                "dir" |
                "colspan" |
                "rowspan" |
                "start" |
                "value" |
                "scope" |
                "aria-label" |
                "datetime" |
                "role" |
                "tabindex"
        ) {
            out.html
                .push_str(&format!(" {attribute}=\"{}\"", escape(value)));
        } else if attribute == "href" {
            let value = if let Some(base) = base {
                let url = base.join(value)?;
                if url.origin() == base.origin() &&
                    url.path() == base.path() &&
                    url.query() == base.query() &&
                    url.fragment().is_some()
                {
                    format!("#{}", url.fragment().unwrap())
                } else {
                    url.to_string()
                }
            } else {
                value.to_owned()
            };
            if !value.starts_with('#') {
                let url = Url::parse(&value)?;
                ensure!(
                    matches!(url.scheme(), "https" | "http" | "mailto"),
                    "unsupported published HTML link"
                );
            }
            out.links.push(value.clone());
            out.html.push_str(&format!(" href=\"{}\"", escape(&value)));
        } else if attribute == "style" {
            ensure!(
                !value.to_lowercase().contains("url") &&
                    !value.to_lowercase().contains("expression") &&
                    !value.contains(['<', '>', '\\']),
                "executable or external HTML styles are unsupported"
            );
            out.html.push_str(&format!(" style=\"{}\"", escape(value)));
        }
    }
    let heading_flags = if matches!(name, "h3" | "h4" | "h5" | "h6") {
        Some(flags(element)?)
    } else {
        None
    };
    if let Some((numbered, toc)) = heading_flags {
        out.html.push_str(&format!(
            " data-grapharch-numbered=\"{numbered}\" \
             data-grapharch-toc=\"{toc}\""
        ));
    }
    out.html.push('>');
    if matches!(name, "h3" | "h4" | "h5" | "h6") {
        let depth = name.as_bytes()[1] as usize - b'2' as usize;
        let id = element
            .value()
            .attr("id")
            .ok_or_else(|| anyhow::anyhow!("native heading requires an id"))?
            .to_owned();
        let text = title(element);
        let (numbered, toc) = heading_flags.unwrap();
        let ordinal = if numbered { number(depth)? } else { None };
        let label = match ordinal {
            Some(number) => format!("{number} {text}"),
            None => text.clone(),
        };
        out.headings
            .push(Heading { depth, id, text, numbered, toc });
        out.html.push_str(&escape(&label));
    } else if name == "pre" {
        // RDF examples sometimes use inert script elements inside <pre>. Their
        // source becomes escaped text; no script element is emitted.
        out.html.push_str("<code>");
        out.html
            .push_str(&escape(&element.text().collect::<String>()));
        out.html.push_str("</code>");
    } else {
        children(element, base, number, out)?;
    }
    if !matches!(name, "br" | "hr" | "col" | "wbr") {
        out.html.push_str(&format!("</{name}>"));
    }
    if wrap_table {
        out.html.push_str("</div>");
    }
    Ok(())
}

pub(crate) fn title(element: ElementRef<'_>) -> String {
    let text = element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if let Some((prefix, title)) = text.split_once(' ') &&
        prefix.bytes().all(|c| {
            c.is_ascii_digit() || c == b'.' || c.is_ascii_uppercase()
        }) &&
        (prefix.contains('.') || prefix.bytes().all(|c| c.is_ascii_digit()))
    {
        return title.trim().to_owned();
    }
    text
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn flags(heading: ElementRef<'_>) -> Result<(bool, bool)> {
    fn attr(node: ElementRef<'_>, key: &str) -> Result<Option<bool>> {
        match node.value().attr(key) {
            None => Ok(None),
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            _ => anyhow::bail!("invalid native heading flag {key}"),
        }
    }
    let numbered = match attr(heading, "data-grapharch-numbered")? {
        Some(value) => value,
        None => {
            heading
                .select(&scraper::Selector::parse(".secno").unwrap())
                .next()
                .is_some()
        },
    };
    let toc = match attr(heading, "data-grapharch-toc")? {
        Some(value) => value,
        None => {
            let parent = heading.parent().and_then(ElementRef::wrap);
            let section = parent.and_then(|parent| {
                if parent
                    .value()
                    .classes()
                    .any(|class| class == "header-wrapper")
                {
                    parent.parent().and_then(ElementRef::wrap)
                } else {
                    Some(parent)
                }
            });
            let first = section
                .filter(|node| node.value().name() == "section")
                .and_then(|section| {
                    section
                        .select(
                            &scraper::Selector::parse("h2,h3,h4,h5,h6")
                                .unwrap(),
                        )
                        .next()
                });
            numbered || first.is_some_and(|node| node.id() == heading.id())
        },
    };
    Ok((numbered, toc))
}

pub(crate) fn example_id(element: ElementRef<'_>) -> String {
    element
        .value()
        .attr("id")
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "grapharch-example-{:x}",
                Sha256::digest(element.html().as_bytes())
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_native_headings_do_not_number_bare_grammar_subtitles()
    -> Result<()> {
        let source =
            "<section id=\"grammar\"><div class=\"header-wrapper\"><h3 \
             id=\"grammar-heading\"><bdi class=\"secno\">5.3 \
             </bdi>Grammar</h3></div><div class=\"header-wrapper\"><h3 \
             id=\"terminals\">Productions for terminals</h3></div></section>";
        let normalized = read(source, None, &mut |_| Ok(None))?;
        let mut numbered = 0;
        let rendered = read(&normalized.html, None, &mut |_| {
            numbered += 1;
            Ok(Some("5.3".into()))
        })?;
        assert_eq!(numbered, 1);
        assert!(rendered.html.contains(">Productions for terminals</h3>"));
        Ok(())
    }
}
