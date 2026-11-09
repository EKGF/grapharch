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
    let mut events: Vec<_> = events.collect();
    let subjects = super::rdf_tables::subjects(&mut events)?;
    for index in 0..events.len().saturating_sub(1) {
        if let Event::Start(Tag::Link { dest_url, .. }) = &events[index] &&
            let Some(label) = crate::publication::badge::label(dest_url)
        {
            anyhow::ensure!(
                matches!(events[index + 1], Event::End(TagEnd::Link)),
                "identifier badges must have no link body"
            );
            events[index] = Event::InlineHtml(
                format!("<span class=\"identifier-badge\">{label}</span>")
                    .into(),
            );
            events[index + 1] = Event::InlineHtml("".into());
        }
    }
    for index in 0..events.len().saturating_sub(1) {
        if let Event::Start(Tag::Link { dest_url, .. }) = &events[index] &&
            let Some(label) = crate::publication::anchor::label(dest_url)
        {
            anyhow::ensure!(
                matches!(events[index + 1], Event::End(TagEnd::Link)),
                "explicit anchor must have no link body"
            );
            events[index] = Event::InlineHtml(
                format!("<span id=\"{label}\"></span>").into(),
            );
            events[index + 1] = Event::InlineHtml("".into());
        }
    }
    for index in 0..events.len().saturating_sub(1) {
        if let Event::Start(Tag::Image { dest_url, .. }) = &events[index] &&
            let Some(icon) =
                crate::publication::icon::Icon::parse_uri(dest_url)?
        {
            anyhow::ensure!(
                matches!(events[index + 1], Event::End(TagEnd::Image)),
                "decorative icons must have empty alternative text"
            );
            events[index] = Event::InlineHtml(icon.html()?.into());
            events[index + 1] = Event::InlineHtml("".into());
        }
    }
    let mut semantic_table = false;
    for index in 0..events.len() {
        if matches!(events[index], Event::Start(Tag::Table(_))) {
            let class = if super::rdf_tables::matches(&events, index) {
                Some("rdf-properties")
            } else {
                None
            };
            semantic_table = class.is_some();
            if let Some(class) = class {
                if let Some(subject) = subjects.get(&index) {
                    events[index + 1] = Event::Html(
                        format!(
                            "<thead>{}<tr>",
                            super::rdf_tables::subject_html(subject)
                        )
                        .into(),
                    );
                }
                events[index] =
                    Event::Html(format!("<table class=\"{class}\">").into());
            }
        } else if semantic_table &&
            matches!(events[index], Event::End(TagEnd::Table))
        {
            events[index] = Event::Html("</table>".into());
            semantic_table = false;
        }
    }
    let events = code::events(events)?;
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
            let table_start = matches!(&event, Event::Start(Tag::Table(_))) ||
                matches!(&event, Event::Html(value) if matches!(value.as_ref(), "<table class=\"rdf-properties\">"));
            let table_end = matches!(&event, Event::End(TagEnd::Table)) ||
                matches!(&event, Event::Html(value) if value.as_ref() == "</table>");
            let before = table_start.then(|| Event::Html("<div class=\"table-scroll\" role=\"region\" aria-label=\"Data table\" tabindex=\"0\">".into()));
            let after = table_end.then(|| Event::Html("</div>".into()));
            before.into_iter().chain(std::iter::once(event)).chain(title).chain(after)
        }))
}

#[cfg(test)]
mod tests {
    use {
        super::events,
        pulldown_cmark::{Parser, html},
    };

    #[test]
    fn tables_have_one_keyboard_scroll_region_and_keep_alignment() {
        let source = "| Column | Value |\n| :--- | ---: |\n| long | text |\n";
        let mut output = String::new();
        html::push_html(
            &mut output,
            events(Parser::new_ext(
                source,
                crate::publication::markdown::options(),
            ))
            .unwrap(),
        );
        assert_eq!(
            output.matches("class=\"table-scroll\"").count(),
            1,
            "{output}"
        );
        assert!(output.contains("tabindex=\"0\""));
        assert!(output.contains("text-align: right"), "{output}");
        assert!(output.ends_with("</table>\n</div>"), "{output}");
    }

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
