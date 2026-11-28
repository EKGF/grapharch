//! Icon shortcodes, as pymdownx.emoji writes them, become decorative builtin
//! image references. An icon the bundle lacks stays literal with a warning.
use {
    super::source::FileInput,
    crate::publication::icon::{Icon, IconPresentation, IconShortcode},
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Quad},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    regex::Regex,
};

/// Why an icon shortcode was not reproduced exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IconWarningCode {
    /// No bundled set holds the named icon; the shortcode stays literal.
    Unknown,
    /// An attribute-list entry has no GraphArch presentation.
    AttributeIgnored,
}
impl IconWarningCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "icon-unknown",
            Self::AttributeIgnored => "icon-attribute-ignored",
        }
    }
}

/// One icon diagnostic retained with the source fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IconWarning {
    pub(super) code:    IconWarningCode,
    pub(super) message: String,
}
impl IconWarning {
    pub(super) fn quads(
        &self,
        input: &FileInput,
        about: &NamedNode,
    ) -> Vec<Quad> {
        let diagnostic =
            crate::publication::dataset::identity("icon-diagnostic", &[
                about.as_str(),
                self.code.as_str(),
                &self.message,
            ]);
        vec![
            input.quad(about, "diagnostic", diagnostic.clone()),
            input.quad(
                &diagnostic,
                "type",
                crate::publication::term("Diagnostic"),
            ),
            input.quad(
                &diagnostic,
                "severity",
                Literal::new_simple_literal("warning"),
            ),
            input.quad(
                &diagnostic,
                "code",
                Literal::new_simple_literal(self.code.as_str()),
            ),
            input.quad(
                &diagnostic,
                "message",
                Literal::new_simple_literal(&self.message),
            ),
            input.quad(&diagnostic, "about", about.clone()),
        ]
    }
}

/// A body with its icon shortcodes replaced, and what could not be.
pub(super) struct Normalized {
    pub(super) body:     String,
    pub(super) warnings: Vec<IconWarning>,
}

pub(super) fn normalize(body: &str) -> Result<Normalized> {
    ensure!(
        body.len() <= 1024 * 1024,
        "inline icon source exceeds 1 MiB"
    );
    let pattern = Regex::new(
        r":((?:material|octicons|simple|fontawesome-(?:brands|regular|solid))-[a-z0-9_-]+):(?:[ \t]*\{:?[ \t]*([^}\n]*?)[ \t]*\})?",
    )?;
    let mut output = String::new();
    let mut warnings = Vec::new();
    let mut offset = 0;
    let mut count = 0;
    let mut code = false;
    for (event, range) in
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                code = true;
                continue
            },
            Event::End(TagEnd::CodeBlock) => {
                code = false;
                continue
            },
            Event::Text(_) if !code => {},
            _ => continue,
        }
        for capture in pattern.captures_iter(&body[range.clone()]) {
            count += 1;
            ensure!(count <= 128, "inline icons exceed 128 per source");
            let matched = capture.get(0).unwrap();
            let start = range.start + matched.start();
            let end = range.start + matched.end();
            let shortcode = IconShortcode::try_new(&capture[1])?;
            let mut presentation = IconPresentation::default();
            for entry in capture
                .get(2)
                .map_or("", |entry| entry.as_str())
                .split_whitespace()
            {
                let applied = entry
                    .strip_prefix('.')
                    .is_some_and(|class| presentation.apply_class(class));
                if !applied {
                    warnings.push(IconWarning {
                        code:    IconWarningCode::AttributeIgnored,
                        message: format!(
                            "icon :{shortcode}: attribute `{entry}` is not \
                             reproduced"
                        ),
                    });
                }
            }
            output.push_str(&body[offset..start]);
            match Icon::from_source(shortcode.clone(), presentation)? {
                Some(icon) => output.push_str(&format!("![]({})", icon.uri())),
                None => {
                    warnings.push(IconWarning {
                        code:    IconWarningCode::Unknown,
                        message: format!(
                            "icon :{shortcode}: is not in the bundled {:?} set",
                            shortcode.set()
                        ),
                    });
                    output.push_str(&body[start..end]);
                },
            }
            offset = end;
        }
    }
    output.push_str(&body[offset..]);
    Ok(Normalized { body: output, warnings })
}
#[cfg(test)]
mod tests {
    use {
        super::{IconWarningCode, normalize},
        pulldown_cmark::{Event, Parser, Tag},
    };
    #[test]
    fn icons_inside_links_keep_labels_and_literal_examples() {
        let input = ":material-cached:{ .lg } \
                     __Deploy__\n\n[:octicons-arrow-right-24: Learn \
                     more](next.md)";
        let output = normalize(input).unwrap();
        assert!(output.warnings.is_empty());
        let output = output.body;
        assert!(output.contains("__Deploy__"));
        assert!(output.contains("Learn more"));
        assert_eq!(
            Parser::new(&output)
                .filter(|event| {
                    matches!(event, Event::Start(Tag::Image { .. }))
                })
                .count(),
            2
        );
        assert_eq!(
            Parser::new(&output)
                .filter(|event| matches!(event, Event::Start(Tag::Link { .. })))
                .count(),
            1
        );
        let code = format!("```md\n{input}\n```");
        assert_eq!(normalize(&code).unwrap().body, code);
    }

    #[test]
    fn unknown_icons_stay_literal_with_a_typed_warning() {
        let output = normalize("a :material-no-such-icon:{ .lg } b").unwrap();
        assert_eq!(output.body, "a :material-no-such-icon:{ .lg } b");
        assert_eq!(output.warnings.len(), 1);
        assert_eq!(output.warnings[0].code, IconWarningCode::Unknown);
        let output = normalize(":simple-rust:{: .lg .middle #id }").unwrap();
        assert!(
            output.body.ends_with("simple-rust:lg:middle)"),
            "{}",
            output.body
        );
        assert_eq!(
            output.warnings[0].code,
            IconWarningCode::AttributeIgnored
        );
    }
}
