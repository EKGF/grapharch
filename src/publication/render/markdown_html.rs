//! Shared presentation of semantic Markdown events after source validation.
use {
    super::callouts,
    pulldown_cmark::{Event, Tag, TagEnd},
    std::collections::HashSet,
};

pub(super) fn events<'a>(
    events: impl Iterator<Item = Event<'a>>,
) -> impl Iterator<Item = Event<'a>> {
    // Bounded by the caller's per-fragment/page Markdown budget. Classify task
    // items structurally so nested and mixed lists retain their semantics.
    let events: Vec<_> = events.collect();
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
    events
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
        })
}
