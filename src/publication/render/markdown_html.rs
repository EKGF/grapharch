//! Shared presentation of semantic Markdown events after source validation.
mod code;

use {
    super::callouts,
    pulldown_cmark::{Event, Tag, TagEnd},
    std::collections::HashSet,
};

pub(super) fn events<'a>(
    events: impl Iterator<Item = Event<'a>>,
) -> anyhow::Result<impl Iterator<Item = Event<'a>>> {
    // Bounded by the caller's per-fragment/page Markdown budget. Classify task
    // items structurally so nested and mixed lists retain their semantics.
    let mut events = code::events(events.collect())?;
    let mut rdf_table = false;
    for index in 0..events.len() {
        if matches!(events[index], Event::Start(Tag::Table(_))) {
            rdf_table = super::rdf_tables::matches(&events, index);
            if rdf_table {
                events[index] =
                    Event::Html("<table class=\"rdf-properties\">".into());
            }
        } else if rdf_table &&
            matches!(events[index], Event::End(TagEnd::Table))
        {
            events[index] = Event::Html("</table>".into());
            rdf_table = false;
        }
    }
    let mut items = Vec::new();
    let mut tasks = HashSet::new();
    for (index, event) in events.iter().enumerate() {
        match event {
            Event::Start(Tag::Item) => items.push(index),
            Event::End(TagEnd::Item) => {
                items.pop();
            },
            Event::TaskListMarker(_) => {
                if let Some(index) = items.last() {
                    tasks.insert(*index);
                }
            },
            _ => {},
        }
    }
    Ok(events
        .into_iter()
        .enumerate()
        .flat_map(move |(index, event)| {
            let title = match &event {
                Event::Start(Tag::BlockQuote(Some(kind))) => {
                    Some(Event::Html(
                        format!(
                            "<p class=\"callout-title\"><strong>{}</strong></\
                             p>\n",
                            callouts::label(*kind)
                        )
                        .into(),
                    ))
                },
                _ => None,
            };
            let event = if tasks.contains(&index) {
                Event::Html("\n<li class=\"task-list-item\">".into())
            } else {
                event
            };
            std::iter::once(event).chain(title)
        }))
}

#[cfg(test)]
mod tests {
    use {
        super::events,
        pulldown_cmark::{Parser, html},
    };

    #[test]
    fn marked_json_examples_retain_callouts_and_colored_tokens() {
        let markdown = "```json example\n{\"title\": \"UK Bonds\", \
                        \"active\": true}\n```\n\n> [!NOTE]\n> Two contexts \
                        describe the same graph.\n";
        let mut output = String::new();
        html::push_html(
            &mut output,
            events(Parser::new_ext(
                markdown,
                crate::publication::markdown::options(),
            ))
            .unwrap(),
        );
        assert!(
            output.contains("class=\"example code-example\""),
            "{output}"
        );
        assert!(
            output.matches("style=\"color:#").count() > 3,
            "{output}"
        );
        assert!(output.contains("markdown-alert-note"), "{output}");
        assert!(output.contains("callout-title"), "{output}");
    }
}
