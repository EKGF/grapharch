//! Typographic symbols for typed character sequences in prose (the
//! `ga:smartSymbols` profile switch): pymdownx.smartsymbols' arrows,
//! marks and fractions, plus em and en dashes. Code is never changed.
use pulldown_cmark::{Event, Tag, TagEnd};

/// Replacements applied in order, longest sequences first.
const SEQUENCES: [(&str, &str); 15] = [
    ("<-->", "↔"),
    ("<==>", "⇔"),
    ("-->", "→"),
    ("<--", "←"),
    ("==>", "⇒"),
    ("<==", "⇐"),
    ("---", "—"),
    ("--", "–"),
    ("(tm)", "™"),
    ("(TM)", "™"),
    ("(c)", "©"),
    ("(C)", "©"),
    ("(r)", "®"),
    ("(R)", "®"),
    ("+/-", "±"),
];

/// Replacements that apply only as whole words.
const WORDS: [(&str, &str); 5] = [
    ("=/=", "≠"),
    ("c/o", "℅"),
    ("1/2", "½"),
    ("1/4", "¼"),
    ("3/4", "¾"),
];

pub(super) struct SmartSymbols;

impl SmartSymbols {
    /// Rewrite the text events of `events` outside code blocks and links.
    pub(super) fn apply(events: &mut [Event<'_>]) {
        let mut code = 0usize;
        for event in events.iter_mut() {
            match event {
                Event::Start(Tag::CodeBlock(_)) => code += 1,
                Event::End(TagEnd::CodeBlock) => code = code.saturating_sub(1),
                Event::Text(text) if code == 0 => {
                    let replaced = Self::text(text);
                    if replaced != text.as_ref() {
                        *text = replaced.into();
                    }
                },
                _ => {},
            }
        }
    }

    fn text(text: &str) -> String {
        let mut value = text.to_owned();
        for (from, to) in SEQUENCES {
            if value.contains(from) {
                value = value.replace(from, to);
            }
        }
        for (from, to) in WORDS {
            value = Self::words(&value, from, to);
        }
        value
    }

    /// Replace `from` where it is not part of a longer word or number.
    fn words(text: &str, from: &str, to: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find(from) {
            let before =
                rest[..at].chars().last().or_else(|| out.chars().last());
            let after = rest[at + from.len()..].chars().next();
            let joined = |c: Option<char>| {
                c.is_some_and(|c| c.is_alphanumeric() || c == '/')
            };
            out.push_str(&rest[..at]);
            if joined(before) || joined(after) {
                out.push_str(from);
            } else {
                out.push_str(to);
            }
            rest = &rest[at + from.len()..];
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::SmartSymbols;

    #[test]
    fn sequences_become_symbols() {
        assert_eq!(
            SmartSymbols::text("many---if not most--use (c) 2026 --> a <--> b"),
            "many—if not most–use © 2026 → a ↔ b"
        );
        assert_eq!(
            SmartSymbols::text("1/2 cup, 11/2, a/1/2"),
            "½ cup, 11/2, a/1/2"
        );
        assert_eq!(SmartSymbols::text("x =/= y (tm)"), "x ≠ y ™");
    }

    #[test]
    fn code_is_unchanged() {
        let mut events: Vec<_> = pulldown_cmark::Parser::new(
            "a -- b `c -- d`\n\n```\ne -- f\n```\n",
        )
        .collect();
        SmartSymbols::apply(&mut events);
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, events.into_iter());
        assert!(
            html.contains("a – b <code>c -- d</code>"),
            "{html}"
        );
        assert!(html.contains("e -- f"), "{html}");
    }
}
