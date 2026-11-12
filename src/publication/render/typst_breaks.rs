//! Presentation-only line-break opportunities inside long identifiers.
//!
//! Long ASCII identifiers have no hyphenation points. The theme's `ga-wrap`
//! joins their pieces with empty inline boxes, Unicode contingent breaks. A
//! U+200B ZERO WIDTH SPACE would also allow the break, but tagged PDF keeps it
//! in the text that readers copy, search and hear, corrupting the identifier.
//! Every piece is a separate text run in the PDF, so pieces follow the
//! identifier's own words rather than single characters.
use super::markup::quote;

/// The longest ASCII token that is set without break opportunities.
const UNBROKEN_TOKEN: usize = 24;

/// The longest run of letters or digits kept as one table piece.
const TABLE_RUN: usize = 8;

/// Text split at the positions where a line may break.
pub(super) struct Breakable<'a> {
    // bounded: at most one piece per character of one bounded literal.
    pieces: Vec<&'a str>,
}

impl<'a> Breakable<'a> {
    /// Table cell text. Within ASCII tokens longer than 24 bytes, a line may
    /// break after punctuation, at lower-to-upper case and letter-digit
    /// transitions, and after eight letters or digits without either. Prose,
    /// text outside tables and non-ASCII grapheme clusters are left untouched.
    pub(super) fn table(value: &'a str, in_table: bool) -> Self {
        let mut breaks = Vec::new();
        if in_table {
            let mut start = 0;
            for token in value.split_inclusive(char::is_whitespace) {
                if token.trim_end().len() > UNBROKEN_TOKEN && token.is_ascii() {
                    Self::token_breaks(token.as_bytes(), start, &mut breaks);
                }
                start += token.len();
            }
        }
        Self::split(value, breaks)
    }

    fn token_breaks(token: &[u8], start: usize, breaks: &mut Vec<usize>) {
        let mut run = 0;
        for (index, pair) in token.windows(2).enumerate() {
            let [before, after] = [pair[0], pair[1]];
            run = if before.is_ascii_alphanumeric() {
                run + 1
            } else {
                0
            };
            if !after.is_ascii_alphanumeric() {
                continue;
            }
            let transition = !before.is_ascii_alphanumeric() ||
                (before.is_ascii_lowercase() && after.is_ascii_uppercase()) ||
                before.is_ascii_digit() != after.is_ascii_digit() ||
                run >= TABLE_RUN;
            if transition {
                breaks.push(start + index + 1);
                run = 0;
            }
        }
    }

    /// An identifier set in prose: a line may break after `/#_-:.` and after
    /// every 24th character.
    pub(super) fn identifier(value: &'a str) -> Self {
        let breaks = value
            .char_indices()
            .enumerate()
            .filter(|(index, (_, ch))| {
                "/#_-:.".contains(*ch) ||
                    index % UNBROKEN_TOKEN == UNBROKEN_TOKEN - 1
            })
            .map(|(_, (offset, ch))| offset + ch.len_utf8())
            .collect();
        Self::split(value, breaks)
    }

    fn split(value: &'a str, breaks: Vec<usize>) -> Self {
        let mut pieces = Vec::with_capacity(breaks.len() + 1);
        let mut start = 0;
        for end in breaks {
            if end > start && end < value.len() {
                pieces.push(&value[start..end]);
                start = end;
            }
        }
        pieces.push(&value[start..]);
        Self { pieces }
    }

    /// Typst markup for ordinary text.
    pub(super) fn text(&self) -> String { self.markup("text", "ga-wrap") }

    /// Typst markup for inline code.
    pub(super) fn raw(&self) -> String { self.markup("raw", "ga-wrap-raw") }

    fn markup(&self, single: &str, wrapped: &str) -> String {
        let arguments = self
            .pieces
            .iter()
            .map(|piece| quote(piece))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "#{}({arguments})",
            if self.pieces.len() == 1 {
                single
            } else {
                wrapped
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Breakable;

    #[test]
    fn table_identifiers_break_at_their_words_without_new_text() {
        let label = "dprod-shapes:RejectMixedLifecycleStatusOnDataOfferShape";
        let wrapped = Breakable::table(label, true);
        assert_eq!(wrapped.pieces.concat(), label);
        assert_eq!(&wrapped.pieces[..5], [
            "dprod-", "shapes:", "Reject", "Mixed", "Lifecycl"
        ]);
        assert!(wrapped.text().starts_with("#ga-wrap(\"dprod-\", "));
        assert!(wrapped.raw().starts_with("#ga-wrap-raw(\"dprod-\", "));
        assert!(!wrapped.text().contains('\u{200b}'));
        let iri = "https://www.omg.org/spec/Commons/RegistrationAuthorities/";
        let pieces = Breakable::table(iri, true).pieces;
        assert!(pieces.contains(&"Registra") && pieces.contains(&"tion"));
        assert!(pieces.iter().all(|piece| piece.len() <= 9));
        assert_eq!(
            Breakable::table(label, false).text(),
            format!("#text(\"{label}\")")
        );
        assert_eq!(
            Breakable::table("ordinary prose and short names", true).raw(),
            "#raw(\"ordinary prose and short names\")"
        );
        let combining = "e\u{301}".repeat(30);
        assert_eq!(Breakable::table(&combining, true).pieces, [
            &combining
        ]);
    }

    #[test]
    fn prose_identifiers_break_after_separators_and_long_runs() {
        let curie = "example:vocabulary_AVeryLongLocalNameWithoutAnySeparators";
        let wrapped = Breakable::identifier(curie);
        assert_eq!(wrapped.pieces.concat(), curie);
        assert_eq!(&wrapped.pieces[..2], ["example:", "vocabulary_"]);
        assert!(
            wrapped
                .pieces
                .iter()
                .all(|piece| piece.chars().count() <= 24)
        );
        assert_eq!(
            Breakable::identifier("short").text(),
            "#text(\"short\")"
        );
        assert_eq!(Breakable::identifier("ends/").pieces, ["ends/"]);
    }
}
