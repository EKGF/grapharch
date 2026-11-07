//! Print projects authored image alternatives onto a light paper background.
use {
    crate::publication::image_hints::{ColorScheme, ImageHints},
    anyhow::Result,
    pulldown_cmark::{Event, Tag, TagEnd},
};
pub(super) fn print_events<'a>(events: &[Event<'a>]) -> Result<Vec<Event<'a>>> {
    let mut output = Vec::new(); // bounded: input is one capped Markdown fragment.
    let mut hiding = false;
    let mut paragraph = None;
    for event in events {
        if hiding {
            if matches!(event, Event::End(TagEnd::Image)) {
                hiding = false;
            }
            continue;
        }
        if let Event::Start(Tag::Image { title, .. }) = event &&
            ImageHints::parse(title)?.color_scheme == Some(ColorScheme::Dark)
        {
            hiding = true;
            continue;
        }
        if matches!(event, Event::Start(Tag::Paragraph)) {
            paragraph = Some(output.len());
        }
        if matches!(event, Event::End(TagEnd::Paragraph)) &&
            let Some(start) = paragraph.take()
        {
            let empty = output[start + 1..].iter().all(|event| {
                match event {
                    Event::SoftBreak | Event::HardBreak => true,
                    Event::Text(text) => text.trim().is_empty(),
                    _ => false,
                }
            });
            if empty {
                output.truncate(start);
                continue;
            }
        }
        output.push(event.clone());
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use {
        super::print_events,
        pulldown_cmark::{Event, Parser, Tag},
    };
    #[test]
    fn print_keeps_light_images_and_text_without_empty_dark_paragraphs() {
        let source = "![Light](light.svg \
                      \"grapharch:only-light\")\n\n![Dark](dark.svg \
                      \"grapharch:only-dark\")\n\nText ![Inline \
                      dark](dark.svg \"grapharch:only-dark\") remains.";
        let events = Parser::new(source).collect::<Vec<_>>();
        let print = print_events(&events).unwrap();
        let images = print
            .iter()
            .filter_map(|event| {
                match event {
                    Event::Start(Tag::Image { dest_url, .. }) => {
                        Some(dest_url.as_ref())
                    },
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(images, ["light.svg"]);
        assert_eq!(
            print
                .iter()
                .filter(|event| matches!(event, Event::Start(Tag::Paragraph)))
                .count(),
            2
        );
        assert!(print.iter().any(|event| matches!(event,Event::Text(text) if text.contains("remains"))));
    }
}
