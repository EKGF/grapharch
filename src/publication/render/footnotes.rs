//! Typed note definitions and native PDF references for one bounded fragment.
use {
    super::markup::{Markdown, quote},
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Tag, TagEnd},
    std::collections::{BTreeMap, BTreeSet},
};

pub(super) struct Notes<'a> {
    definitions: BTreeMap<String, Vec<Event<'a>>>,
    emitted:     BTreeSet<String>,
}

impl<'a> Notes<'a> {
    pub fn read(events: &[Event<'a>]) -> Result<(Self, Vec<Event<'a>>)> {
        let mut definitions = BTreeMap::new(); // bounded: 128 definitions per fragment.
        let mut body = Vec::new(); // bounded by capped input fragment events.
        let mut current: Option<(String, Vec<Event<'a>>)> = None;
        for event in events {
            match event {
                Event::Start(Tag::FootnoteDefinition(label)) => {
                    ensure!(
                        current.is_none(),
                        "nested note definitions are unsupported"
                    );
                    current = Some((label.to_string(), Vec::new()));
                },
                Event::End(TagEnd::FootnoteDefinition) => {
                    let (label, events) = current.take().ok_or_else(|| {
                        anyhow::anyhow!("unmatched footnote boundary")
                    })?;
                    ensure!(
                        definitions.insert(label, events).is_none(),
                        "duplicate retained footnote definition"
                    );
                    ensure!(
                        definitions.len() <= 128,
                        "fragment exceeds 128 footnotes"
                    );
                },
                Event::FootnoteReference(_) if current.is_some() => {
                    anyhow::bail!("nested footnote references are unsupported")
                },
                _ => {
                    if let Some((_, events)) = &mut current {
                        events.push(event.clone());
                    } else {
                        body.push(event.clone());
                    }
                },
            }
        }
        ensure!(
            current.is_none(),
            "unterminated footnote definition"
        );
        Ok((
            Self { definitions, emitted: BTreeSet::new() },
            body,
        ))
    }

    pub fn typst(
        &mut self,
        label: &str,
        markdown: &Markdown<'a>,
    ) -> Result<String> {
        let events = self.definitions.get(label).ok_or_else(|| {
            anyhow::anyhow!("missing retained footnote definition {label}")
        })?;
        if !self.emitted.insert(label.into()) {
            return Ok(format!("#footnote(label({}))", quote(label)));
        }
        let mut events = events.clone();
        // Typst supplies the note's leading paragraph beside its number.
        if matches!(events.first(), Some(Event::Start(Tag::Paragraph))) {
            events.remove(0);
            let end = events
                .iter()
                .position(|event| {
                    matches!(event, Event::End(TagEnd::Paragraph))
                })
                .ok_or_else(|| {
                    anyhow::anyhow!("unterminated note paragraph")
                })?;
            events.remove(end);
        }
        let body =
            super::typst_markdown::convert(&markdown.with_events(events))?;
        Ok(format!(
            "#footnote[{body}]#label({})",
            quote(label)
        ))
    }
}
