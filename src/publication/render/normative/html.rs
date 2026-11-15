//! HTML presentation of declared normative keywords.
use {
    super::{
        Conformance,
        keyword::{Segment, Segments},
        runs::Protected,
    },
    pulldown_cmark::Event,
};

/// The element and class W3C specifications use for BCP 14 keywords.
pub(super) const OPEN: &str = "<em class=\"rfc2119\">";
pub(super) const CLOSE: &str = "</em>";

impl Conformance {
    /// Wrap each standalone keyword of unprotected prose in
    /// `<em class="rfc2119">`. Raw HTML events, including native markup that
    /// already carries the class, pass through unchanged.
    pub(in crate::publication::render) fn html<'a>(
        self,
        events: impl IntoIterator<Item = Event<'a>>,
    ) -> Vec<Event<'a>> {
        let events = self.coalesce(events.into_iter().collect(), "\n");
        if !self.bcp14() {
            return events;
        }
        // bounded: one capped fragment, three events per keyword.
        let mut output = Vec::with_capacity(events.len());
        let mut protected = Protected::default();
        for event in events {
            let inside = protected.observe(&event);
            let Event::Text(text) = &event else {
                output.push(event);
                continue;
            };
            if inside || !Segments::any(text) {
                output.push(event);
                continue;
            }
            for segment in Segments::new(text) {
                match segment {
                    Segment::Prose(prose) => {
                        output.push(Event::Text(prose.to_owned().into()))
                    },
                    Segment::Keyword(_, spelling) => {
                        output.push(Event::InlineHtml(OPEN.into()));
                        output.push(Event::Text(spelling.to_owned().into()));
                        output.push(Event::InlineHtml(CLOSE.into()));
                    },
                }
            }
        }
        output
    }
}
