//! The BCP 14 requirement-level keywords and their lexical recognition.
//!
//! RFC 8174 clarifies that only the all-uppercase spellings carry BCP 14
//! meaning, so lowercase and mixed-case words are ordinary prose.

/// One BCP 14 keyword, as defined by RFC 2119 and clarified by RFC 8174.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::publication::render) enum Keyword {
    Must,
    MustNot,
    Required,
    Shall,
    ShallNot,
    Should,
    ShouldNot,
    Recommended,
    NotRecommended,
    May,
    Optional,
}

impl Keyword {
    /// bounded: the fixed vocabulary. Multi-word forms precede the single
    /// words they start with, so the longest form wins.
    const ALL: [Self; 11] = [
        Self::MustNot,
        Self::ShallNot,
        Self::ShouldNot,
        Self::NotRecommended,
        Self::Must,
        Self::Required,
        Self::Shall,
        Self::Should,
        Self::Recommended,
        Self::May,
        Self::Optional,
    ];

    /// The words of the keyword, separated by single spaces.
    pub(in crate::publication::render) fn as_str(self) -> &'static str {
        match self {
            Self::Must => "MUST",
            Self::MustNot => "MUST NOT",
            Self::Required => "REQUIRED",
            Self::Shall => "SHALL",
            Self::ShallNot => "SHALL NOT",
            Self::Should => "SHOULD",
            Self::ShouldNot => "SHOULD NOT",
            Self::Recommended => "RECOMMENDED",
            Self::NotRecommended => "NOT RECOMMENDED",
            Self::May => "MAY",
            Self::Optional => "OPTIONAL",
        }
    }

    /// The byte length of this keyword at the start of `text`, when it is
    /// spelled there. Words of a multi-word keyword are separated by any
    /// nonempty run of whitespace, including a soft line break.
    fn spelled_at(self, text: &str) -> Option<usize> {
        let mut length = 0;
        for (index, word) in self.as_str().split(' ').enumerate() {
            let rest = &text[length..];
            let rest = if index == 0 {
                rest
            } else {
                let trimmed = rest.trim_start();
                if trimmed.len() == rest.len() {
                    return None;
                }
                length += rest.len() - trimmed.len();
                trimmed
            };
            if !rest.starts_with(word) {
                return None;
            }
            length += word.len();
        }
        Some(length)
    }

    /// The longest keyword standing alone at the start of `text`.
    fn standalone_at(text: &str) -> Option<(Self, usize)> {
        Self::ALL.into_iter().find_map(|keyword| {
            keyword
                .spelled_at(text)
                .filter(|&length| {
                    !text[length..].chars().next().is_some_and(Self::word)
                })
                .map(|length| (keyword, length))
        })
    }

    /// Characters that join a candidate to a longer word or a hyphenated
    /// compound, such as `NON-OPTIONAL`.
    fn word(character: char) -> bool {
        character.is_alphanumeric() || matches!(character, '_' | '-')
    }
}

/// A run of prose or one recognized keyword, borrowed from scanned text.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::publication::render) enum Segment<'a> {
    Prose(&'a str),
    /// A keyword with its source spelling, which may contain a line break.
    Keyword(Keyword, &'a str),
}

/// The segments of one text run, in source order.
pub(in crate::publication::render) struct Segments<'a> {
    text:   &'a str,
    offset: usize,
}

impl<'a> Segments<'a> {
    pub(in crate::publication::render) fn new(text: &'a str) -> Self {
        Self { text, offset: 0 }
    }

    /// Whether the text contains at least one standalone keyword.
    pub(in crate::publication::render) fn any(text: &str) -> bool {
        Segments::new(text)
            .any(|segment| matches!(segment, Segment::Keyword(..)))
    }
}

impl<'a> Iterator for Segments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.text;
        if self.offset >= text.len() {
            return None;
        }
        let mut previous = text[..self.offset]
            .chars()
            .next_back()
            .filter(|&c| Keyword::word(c));
        for (relative, character) in text[self.offset..].char_indices() {
            let position = self.offset + relative;
            if previous.is_none() &&
                let Some((keyword, length)) =
                    Keyword::standalone_at(&text[position..])
            {
                if position > self.offset {
                    let prose = &text[self.offset..position];
                    self.offset = position;
                    return Some(Segment::Prose(prose));
                }
                self.offset = position + length;
                return Some(Segment::Keyword(
                    keyword,
                    &text[position..position + length],
                ));
            }
            previous = Some(character).filter(|&c| Keyword::word(c));
        }
        let prose = &text[self.offset..];
        self.offset = text.len();
        Some(Segment::Prose(prose))
    }
}
