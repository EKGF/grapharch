//! Authored table captions: a paragraph `Table: <caption>` immediately before
//! a Markdown table captions that table, as in Pandoc. Renderers number the
//! captioned table and place the caption by their theme.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Tag, TagEnd},
    std::collections::BTreeMap,
};

/// The marker that opens a caption paragraph.
const MARKER: &str = "Table:";
/// Longest accepted caption, in bytes.
const LIMIT: usize = 512;

/// The caption text of one authored table.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TableCaption(String);

impl TableCaption {
    /// The caption that a paragraph spanning `events` declares, if its only
    /// content is text opening with the marker.
    fn read(events: &[Event<'_>]) -> Result<Option<Self>> {
        let mut text = String::new();
        for event in events {
            match event {
                Event::Text(value) => text.push_str(value),
                Event::SoftBreak => text.push(' '),
                _ => return Ok(None),
            }
        }
        let Some(caption) = text.strip_prefix(MARKER) else {
            return Ok(None);
        };
        let caption = caption.trim();
        ensure!(
            !caption.is_empty() && caption.len() <= LIMIT,
            "a table caption needs 1 to {LIMIT} bytes of text"
        );
        Ok(Some(Self(caption.to_owned())))
    }

    pub fn text(&self) -> &str { &self.0 }
}

/// Captions keyed by the index of the `Start(Table)` event they caption.
#[derive(Default)]
pub(super) struct TableCaptions(BTreeMap<usize, TableCaption>);

impl TableCaptions {
    /// Remove every caption paragraph from `events`, keeping its caption for
    /// the table that follows it.
    pub fn take(events: &mut Vec<Event<'_>>) -> Result<Self> {
        let mut captions = BTreeMap::new();
        let mut index = 0;
        while index < events.len() {
            if matches!(events[index], Event::Start(Tag::Paragraph)) &&
                let Some(end) = events[index..]
                    .iter()
                    .position(|event| {
                        matches!(event, Event::End(TagEnd::Paragraph))
                    })
                    .map(|offset| index + offset) &&
                matches!(
                    events.get(end + 1),
                    Some(Event::Start(Tag::Table(_)))
                ) &&
                let Some(caption) =
                    TableCaption::read(&events[index + 1..end])?
            {
                events.drain(index..=end);
                captions.insert(index, caption);
            }
            index += 1;
        }
        Ok(Self(captions))
    }

    /// The caption of the table starting at `index`.
    pub fn get(&self, index: usize) -> Option<&TableCaption> {
        self.0.get(&index)
    }

    /// Wrap each captioned table in an HTML figure with its caption first.
    pub fn html(self, events: &mut Vec<Event<'_>>) -> Result<()> {
        for (index, caption) in self.0.into_iter().rev() {
            let end = events[index..]
                .iter()
                .position(|event| matches!(event, Event::End(TagEnd::Table)))
                .map(|offset| index + offset)
                .ok_or_else(|| {
                    anyhow::anyhow!("unterminated captioned table")
                })?;
            events.insert(end + 1, Event::Html("</figure>\n".into()));
            events.insert(
                index,
                Event::Html(
                    format!(
                        "<figure class=\"ga-table-figure\"><figcaption>{}</\
                         figcaption>\n",
                        super::markup::html_escape(caption.text())
                    )
                    .into(),
                ),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        pulldown_cmark::{Parser, html},
    };

    fn events(source: &str) -> Vec<Event<'_>> {
        Parser::new_ext(source, crate::publication::markdown::options())
            .collect()
    }

    #[test]
    fn a_marked_paragraph_before_a_table_captions_it() {
        let mut events = events(
            "Table: Prefixes & namespaces\n\n| a | b |\n| - | - |\n| 1 | 2 |\n",
        );
        let captions = TableCaptions::take(&mut events).unwrap();
        assert!(matches!(events[0], Event::Start(Tag::Table(_))));
        assert_eq!(
            captions.get(0).unwrap().text(),
            "Prefixes & namespaces"
        );
        captions.html(&mut events).unwrap();
        let mut output = String::new();
        html::push_html(&mut output, events.into_iter());
        assert!(output.starts_with(
            "<figure class=\"ga-table-figure\"><figcaption>Prefixes &amp; \
             namespaces</figcaption>\n<table>"
        ));
        assert!(output.trim_end().ends_with("</table>\n</figure>"));
    }

    #[test]
    fn other_paragraphs_and_detached_markers_stay_text() {
        for source in [
            "Table: not followed by a table\n\nText.\n",
            "A *Table:* emphasised\n\n| a |\n| - |\n| 1 |\n",
            "Ordinary text\n\n| a |\n| - |\n| 1 |\n",
        ] {
            let mut events = events(source);
            let before = events.len();
            let captions = TableCaptions::take(&mut events).unwrap();
            assert_eq!(events.len(), before, "{source}");
            assert!(captions.0.is_empty(), "{source}");
        }
    }

    #[test]
    fn an_empty_caption_fails() {
        let mut events = events("Table:\n\n| a |\n| - |\n| 1 |\n");
        assert!(TableCaptions::take(&mut events).is_err());
    }
}
