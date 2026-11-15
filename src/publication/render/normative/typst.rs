//! Typst presentation of declared normative keywords. The shared theme's
//! `ga-normative` function applies the theme's keyword style.
use super::{
    super::typst_breaks::Breakable,
    Conformance,
    keyword::{Segment, Segments},
};

impl Conformance {
    /// Typst markup for one unprotected Markdown text event.
    pub(in crate::publication::render) fn typst(
        self,
        text: &str,
        in_table: bool,
    ) -> String {
        if !self.bcp14() {
            return Breakable::table(text, in_table).text();
        }
        Segments::new(text)
            .map(|segment| {
                match segment {
                    Segment::Prose(prose) => {
                        Breakable::table(prose, in_table).text()
                    },
                    Segment::Keyword(keyword, _) => {
                        Self::styled(keyword.as_str())
                    },
                }
            })
            .collect()
    }

    /// Typst markup for native `<em class="rfc2119">` content: styled as a
    /// keyword when declared, otherwise the emphasis it has always been.
    pub(in crate::publication::render) fn native(
        self,
        content: &str,
    ) -> String {
        if self.bcp14() {
            Self::styled(content)
        } else {
            format!("#emph[{content}]")
        }
    }

    /// A closed code block, so that following text cannot continue the call.
    fn styled(markup: &str) -> String { format!("#{{ga-normative[{markup}]}}") }
}
