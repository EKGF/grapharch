//! Allowlist validation for `<foreignObject>` label content in SVG images.
//!
//! Diagram editors such as draw.io write HTML-formatted labels as XHTML
//! inside `<foreignObject>`, often without a plain SVG text fallback. Such
//! content is accepted only when every element, attribute and style
//! declaration is on a passive formatting allowlist: no scripts, event
//! handlers, external references, forms, embedded media or style URLs.
use anyhow::{Result, bail, ensure};

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const SVG: &str = "http://www.w3.org/2000/svg";
/// SVG 1.1 feature strings name capabilities; they are never fetched.
const SVG11_FEATURE: &str = "http://www.w3.org/TR/SVG11/feature#";

/// A passive XHTML formatting element permitted in a label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabelElement {
    Block,
    Inline,
    Break,
    List,
    Table,
}

impl LabelElement {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "div" | "p" | "blockquote" | "pre" | "h1" | "h2" | "h3" |
            "h4" | "h5" | "h6" => Self::Block,
            "span" | "b" | "i" | "u" | "s" | "em" | "strong" | "small" |
            "sub" | "sup" | "code" | "font" | "strike" | "mark" | "abbr" => {
                Self::Inline
            },
            "br" | "hr" | "wbr" => Self::Break,
            "ul" | "ol" | "li" | "dl" | "dt" | "dd" => Self::List,
            "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" |
            "caption" | "colgroup" | "col" => Self::Table,
            _ => return None,
        })
    }

    fn permits(self, attribute: &str) -> bool {
        matches!(
            attribute,
            "style" | "class" | "dir" | "lang" | "title" | "align"
        ) || match self {
            Self::Inline => matches!(attribute, "color" | "face" | "size"),
            Self::Table => {
                matches!(
                    attribute,
                    "colspan" |
                        "rowspan" |
                        "valign" |
                        "width" |
                        "height" |
                        "border" |
                        "cellpadding" |
                        "cellspacing"
                )
            },
            Self::List => matches!(attribute, "start" | "type" | "value"),
            Self::Block | Self::Break => false,
        }
    }
}

/// A validated `<foreignObject>` subtree.
pub(crate) struct ForeignObject;

impl ForeignObject {
    /// Validate one `<foreignObject>` element and everything beneath it.
    pub(crate) fn validate(element: roxmltree::Node<'_, '_>) -> Result<()> {
        for attribute in element.attributes() {
            let name = attribute.name().to_ascii_lowercase();
            let value = attribute.value();
            match name.as_str() {
                "x" | "y" | "width" | "height" | "transform" | "class" |
                "id" | "overflow" | "pointer-events" | "opacity" => {
                    Self::plain(value)?
                },
                "style" => Self::style(value)?,
                "requiredfeatures" => {
                    ensure!(
                        value.split_whitespace().all(|feature| {
                            feature.starts_with(SVG11_FEATURE) &&
                                feature[SVG11_FEATURE.len()..]
                                    .bytes()
                                    .all(|b| b.is_ascii_alphanumeric())
                        }),
                        "SVG foreignObject requires only SVG 1.1 feature \
                         strings"
                    )
                },
                _ => bail!("unsupported SVG foreignObject attribute: {name}"),
            }
        }
        for node in element.descendants().skip(1) {
            if node.is_text() || node.is_comment() {
                continue;
            }
            ensure!(
                node.is_element(),
                "SVG foreignObject supports only XHTML elements and text"
            );
            Self::label_element(node)?;
        }
        Ok(())
    }

    fn label_element(node: roxmltree::Node<'_, '_>) -> Result<()> {
        let tag = node.tag_name();
        ensure!(
            tag.namespace() == Some(XHTML),
            "SVG foreignObject content must be XHTML, not {}",
            tag.namespace().unwrap_or("an unqualified element")
        );
        let name = tag.name().to_ascii_lowercase();
        let Some(element) = LabelElement::parse(&name) else {
            bail!("unsupported XHTML element in SVG foreignObject: {name}")
        };
        for attribute in node.attributes() {
            ensure!(
                attribute
                    .namespace()
                    .is_none_or(|ns| ns == XHTML || ns == SVG),
                "unsupported namespaced attribute in SVG foreignObject: {}",
                attribute.name()
            );
            let attr = attribute.name().to_ascii_lowercase();
            ensure!(
                element.permits(&attr),
                "unsupported XHTML attribute in SVG foreignObject: \
                 {name}@{attr}"
            );
            if attr == "style" {
                Self::style(attribute.value())?;
            } else {
                Self::plain(attribute.value())?;
            }
        }
        Ok(())
    }

    /// Presentation values never reference, escape or execute anything.
    fn plain(value: &str) -> Result<()> {
        let lower = value.to_ascii_lowercase();
        ensure!(
            !value.contains(['\\', '<', '>']) &&
                !lower.contains(':') &&
                !lower.contains("url(") &&
                !lower.contains("expression"),
            "unsafe value in SVG foreignObject: {value}"
        );
        Ok(())
    }

    /// Inline CSS declarations without URLs, escapes, imports or bindings.
    fn style(value: &str) -> Result<()> {
        let compact = value
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        ensure!(
            !value.contains(['\\', '<', '>', '@']) &&
                ![
                    "url(",
                    "image(",
                    "image-set(",
                    "expression(",
                    "javascript:",
                    "behavior:",
                    "-moz-binding",
                    "src(",
                ]
                .iter()
                .any(|token| compact.contains(token)),
            "unsafe style in SVG foreignObject: {value}"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "svg_foreign_object_tests.rs"]
mod tests;
