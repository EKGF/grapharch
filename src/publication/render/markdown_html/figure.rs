//! Standalone images hinted as figures become captioned HTML figures.
use {
    crate::publication::image_hints::ImageHints,
    anyhow::Result,
    pulldown_cmark::{CowStr, Event, Tag, TagEnd},
};

/// Wrap each paragraph holding only a `grapharch:figure` image in a
/// `<figure>` whose caption is the image's alternative text.
pub(super) fn events(events: &mut [Event<'_>]) -> Result<()> {
    for index in 0..events.len().saturating_sub(1) {
        if !matches!(events[index], Event::Start(Tag::Paragraph)) {
            continue;
        }
        let Event::Start(Tag::Image { title, .. }) = &events[index + 1] else {
            continue;
        };
        let hints = ImageHints::parse(title)?;
        if !hints.figure {
            continue;
        }
        let Some(end) = events[index + 1..]
            .iter()
            .position(|event| matches!(event, Event::End(TagEnd::Image)))
            .map(|offset| index + 1 + offset)
        else {
            anyhow::bail!("unterminated figure image");
        };
        anyhow::ensure!(
            matches!(
                events.get(end + 1),
                Some(Event::End(TagEnd::Paragraph))
            ),
            "a figure image must be the only content of its paragraph"
        );
        let mut caption = String::new();
        for event in &events[index + 2..end] {
            if let Event::Text(text) | Event::Code(text) = event {
                caption.push_str(text);
            }
        }
        if let Event::Start(Tag::Image { title, .. }) = &mut events[index + 1] {
            *title = CowStr::from(hints.title.unwrap_or_default());
        }
        events[index] = Event::Html("<figure class=\"ga-figure\">".into());
        events[end + 1] = Event::Html(
            format!(
                "<figcaption>{}</figcaption></figure>\n",
                super::super::markup::html_escape(&caption)
            )
            .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pulldown_cmark::{Parser, html};

    #[test]
    fn a_figure_image_is_captioned_by_its_alternative_text() {
        let mut events: Vec<_> = Parser::new(
            "![Overview & more](a.png \"grapharch:figure\")\n\n![Plain](b.png)",
        )
        .collect();
        super::events(&mut events).unwrap();
        let mut output = String::new();
        html::push_html(&mut output, events.into_iter());
        assert!(output.contains(
            "<figure class=\"ga-figure\"><img src=\"a.png\" alt=\"Overview \
             &amp; more\" /><figcaption>Overview &amp; \
             more</figcaption></figure>"
        ));
        assert!(output.contains("<p><img src=\"b.png\" alt=\"Plain\" /></p>"));
    }
}
