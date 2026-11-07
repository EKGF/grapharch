//! Shared GitHub-flavoured Markdown parsing options for authored documentation.
use pulldown_cmark::{Event, Options, Tag, TagEnd};

pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES |
        Options::ENABLE_STRIKETHROUGH |
        Options::ENABLE_TASKLISTS |
        Options::ENABLE_GFM
}

/// Normalize semantic HTML spellings accepted in Markdown without allowing raw
/// markup through. Comments stay in source provenance but have no visible body.
pub(crate) fn semantic_events(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    // bounded: caller enforces its per-fragment Markdown budget.
    events
        .into_iter()
        .filter_map(|event| {
            match event {
                Event::Start(Tag::HtmlBlock) |
                Event::End(TagEnd::HtmlBlock) => None,
                Event::Html(ref text) | Event::InlineHtml(ref text) => {
                    let value = text.trim();
                    if value
                        .strip_prefix("<!--")
                        .and_then(|rest| rest.split_once("-->"))
                        .is_some_and(|(_, rest)| rest.trim().is_empty())
                    {
                        None
                    } else if ["<br>", "<br/>", "<br />"]
                        .iter()
                        .any(|tag| value.eq_ignore_ascii_case(tag))
                    {
                        Some(Event::HardBreak)
                    } else {
                        Some(event)
                    }
                },
                _ => Some(event),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, pulldown_cmark::Parser};

    #[test]
    fn semantic_html_preserves_code_and_rejectable_markup() {
        let events = semantic_events(
            Parser::new(
                "a<br/>b\n\n<!-- hidden \
                 -->\n\n`<br/>`\n\n<script>bad</script>\n",
            )
            .collect(),
        );
        assert!(events.iter().any(|event| matches!(event, Event::HardBreak)));
        assert!(events.iter().any(|event| matches!(event, Event::Code(text) if text.as_ref() == "<br/>")));
        assert!(!events.iter().any(|event| matches!(event, Event::Html(text) if text.contains("hidden"))));
        assert!(events.iter().any(|event| matches!(event, Event::Html(text) if text.contains("script"))));
    }
}
