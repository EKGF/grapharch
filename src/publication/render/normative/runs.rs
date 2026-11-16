//! Inline prose runs. Markdown splits one sentence into several text events
//! and soft line breaks, and a multi-word keyword can straddle them.
use {
    super::{Conformance, keyword::Segments},
    pulldown_cmark::{Event, Tag, TagEnd},
};

/// Contexts whose text is never presented as a normative keyword: code,
/// image alternative text, headings (whose anchors derive from their text)
/// metadata blocks, and keywords already inside `<em class="rfc2119">`.
#[derive(Default)]
pub(super) struct Protected {
    depth:  usize,
    native: bool,
}

impl Protected {
    /// Track one event; true while the event lies inside a protected context.
    pub(super) fn observe(&mut self, event: &Event<'_>) -> bool {
        match event {
            Event::Start(
                Tag::CodeBlock(_) |
                Tag::Image { .. } |
                Tag::Heading { .. } |
                Tag::MetadataBlock(_),
            ) => {
                self.depth += 1;
                true
            },
            Event::End(
                TagEnd::CodeBlock |
                TagEnd::Image |
                TagEnd::Heading(_) |
                TagEnd::MetadataBlock(_),
            ) => {
                self.depth = self.depth.saturating_sub(1);
                true
            },
            Event::InlineHtml(html) | Event::Html(html) => {
                match html.trim() {
                    super::html::OPEN => self.native = true,
                    super::html::CLOSE if self.native => self.native = false,
                    _ => {},
                }
                true
            },
            _ => self.depth != 0 || self.native,
        }
    }
}

impl Conformance {
    /// Join each unprotected run of text and soft breaks that contains a
    /// keyword into one text event, writing soft breaks as `soft_break`.
    /// Runs without a keyword keep their original events.
    pub(in crate::publication::render) fn coalesce<'a>(
        self,
        events: Vec<Event<'a>>,
        soft_break: &str,
    ) -> Vec<Event<'a>> {
        if !self.bcp14() {
            return events;
        }
        // bounded: one capped fragment's events.
        let mut output = Vec::with_capacity(events.len());
        let mut run: Vec<Event<'a>> = Vec::new(); // bounded: one inline run.
        let mut protected = Protected::default();
        for event in events {
            let inside = protected.observe(&event);
            if !inside && matches!(event, Event::Text(_) | Event::SoftBreak) {
                run.push(event);
                continue;
            }
            Self::flush(&mut run, &mut output, soft_break);
            output.push(event);
        }
        Self::flush(&mut run, &mut output, soft_break);
        output
    }

    fn flush<'a>(
        run: &mut Vec<Event<'a>>,
        output: &mut Vec<Event<'a>>,
        soft_break: &str,
    ) {
        let text: String = run
            .iter()
            .map(|event| {
                match event {
                    Event::Text(text) => text.as_ref(),
                    _ => soft_break,
                }
            })
            .collect();
        if run.len() > 1 && Segments::any(&text) {
            output.push(Event::Text(text.into()));
            run.clear();
        } else {
            output.append(run);
        }
    }
}
