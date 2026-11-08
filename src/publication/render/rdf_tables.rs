//! Shared semantics for the generated RDF predicate/value table projection.
use pulldown_cmark::{Event, Tag, TagEnd};

pub(super) fn matches(events: &[Event<'_>], index: usize) -> bool {
    let Some(header) = events.get(index + 1..index + 9) else {
        return false
    };
    matches!(header, [Event::Start(Tag::TableHead), Event::Start(Tag::TableCell), Event::Text(predicate), Event::End(TagEnd::TableCell), Event::Start(Tag::TableCell), Event::Text(value), Event::End(TagEnd::TableCell), Event::End(TagEnd::TableHead)] if predicate.as_ref() == "RDF predicate" && value.as_ref() == "Value")
}

/// Recognize the generated SHACL summary, without affecting arbitrary tables.
pub(super) fn shacl_summary(events: &[Event<'_>], index: usize) -> bool {
    let Some(Event::Start(Tag::TableHead)) = events.get(index + 1) else {
        return false;
    };
    let mut cursor = index + 2;
    for expected in ["Property", "Value type", "Count", "Source"] {
        let Some(
            [
                Event::Start(Tag::TableCell),
                Event::Text(label),
                Event::End(TagEnd::TableCell),
            ],
        ) = events.get(cursor..cursor + 3)
        else {
            return false;
        };
        if label.as_ref() != expected {
            return false;
        }
        cursor += 3;
    }
    matches!(
        events.get(cursor),
        Some(Event::End(TagEnd::TableHead))
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        pulldown_cmark::{Options, Parser},
    };
    #[test]
    fn only_exact_semantic_header_pair_selects_rdf_layout() {
        for (source, expected) in [
            (
                "| RDF predicate | Value |\n| --- | --- |\n| x | y |",
                true,
            ),
            (
                "| Name | Value |\n| --- | --- |\n| x | y |",
                false,
            ),
            (
                "| RDF predicate | Value | Extra |\n| --- | --- | --- |\n| x \
                 | y | z |",
                false,
            ),
        ] {
            let events: Vec<_> =
                Parser::new_ext(source, Options::ENABLE_TABLES).collect();
            assert_eq!(matches(&events, 0), expected);
        }
    }
}
