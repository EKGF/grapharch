//! RFC 4647 basic language ranges for selecting source annotations.
use {
    anyhow::{Result, ensure},
    oxrdf::Literal,
};

/// A basic language range (RFC 4647 §2.1), such as `en`. It matches a
/// language tag equal to the range or beginning with the range followed by
/// `-`, ignoring ASCII case: `en` matches `en`, `en-GB` and `en-US`, but not
/// `eng`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LanguageRange(String);

impl LanguageRange {
    /// The English range used by source-reference documentation.
    pub(crate) fn english() -> Self { Self("en".into()) }

    /// Validate a basic language range: one to eight ASCII letters, then
    /// subtags of one to eight ASCII alphanumerics.
    pub(crate) fn try_new(range: &str) -> Result<Self> {
        let mut subtags = range.split('-');
        let primary = subtags.next().unwrap_or_default();
        ensure!(
            (1..=8).contains(&primary.len()) &&
                primary.bytes().all(|byte| byte.is_ascii_alphabetic()) &&
                subtags.all(|subtag| {
                    (1..=8).contains(&subtag.len()) &&
                        subtag
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric())
                }),
            "invalid basic language range {range:?}"
        );
        Ok(Self(range.to_ascii_lowercase()))
    }

    /// The range text, in lower case.
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str { &self.0 }

    /// Whether `tag` is selected by basic filtering.
    pub(crate) fn matches_tag(&self, tag: &str) -> bool {
        tag.len() >= self.0.len() &&
            tag.is_char_boundary(self.0.len()) &&
            tag[..self.0.len()].eq_ignore_ascii_case(&self.0) &&
            (tag.len() == self.0.len() ||
                tag.as_bytes()[self.0.len()] == b'-')
    }

    /// Whether a selected literal is language-neutral or tagged exactly with
    /// the range, rather than with a regional or other subtag of it.
    pub(crate) fn is_exact(&self, literal: &Literal) -> bool {
        literal
            .language()
            .is_none_or(|tag| tag.eq_ignore_ascii_case(&self.0))
    }

    /// Of the selected literals, the exact ones when any exist, otherwise
    /// every selected literal: a single-valued title in `en` is not made
    /// ambiguous by its `en-US` variant, yet `en-GB` alone still qualifies.
    pub(crate) fn preferred<'a>(
        &self,
        literals: impl IntoIterator<Item = &'a Literal>,
    ) -> Vec<&'a Literal> {
        let selected: Vec<_> =
            literals.into_iter().filter(|l| self.selects(l)).collect();
        if selected.iter().any(|l| self.is_exact(l)) {
            selected.into_iter().filter(|l| self.is_exact(l)).collect()
        } else {
            selected
        }
    }

    /// Whether a source literal is selected: language-neutral text, or text
    /// whose language tag this range matches.
    pub(crate) fn selects(&self, literal: &Literal) -> bool {
        literal.language().is_none_or(|tag| self.matches_tag(tag))
    }
}

/// A language tag in the case conventions of RFC 5646 §2.1.1, for display:
/// RDF stores tags case-insensitively, so `en-gb` is shown as `en-GB`.
pub(crate) struct LanguageTag(String);

impl LanguageTag {
    pub(crate) fn new(tag: &str) -> Self {
        let mut extension = false;
        let subtags = tag
            .split('-')
            .enumerate()
            .map(|(index, subtag)| {
                if index == 0 || extension {
                    subtag.to_ascii_lowercase()
                } else if subtag.len() == 1 {
                    // Singletons start extensions and private use, whose
                    // subtags keep lower case.
                    extension = true;
                    subtag.to_ascii_lowercase()
                } else if subtag.len() == 2 {
                    subtag.to_ascii_uppercase()
                } else if subtag.len() == 4 {
                    let lower = subtag.to_ascii_lowercase();
                    lower[..1].to_ascii_uppercase() + &lower[1..]
                } else {
                    subtag.to_ascii_lowercase()
                }
            })
            .collect::<Vec<_>>();
        Self(subtags.join("-"))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::{LanguageRange, LanguageTag};

    #[test]
    fn language_tags_display_in_conventional_case() {
        for (tag, shown) in [
            ("en-gb", "en-GB"),
            ("EN-US", "en-US"),
            ("zh-hant-tw", "zh-Hant-TW"),
            ("en-x-gb", "en-x-gb"),
            ("es-419", "es-419"),
        ] {
            assert_eq!(LanguageTag::new(tag).as_str(), shown);
        }
    }

    #[test]
    fn basic_filtering_matches_regional_variants_only_at_subtag_boundaries() {
        let english = LanguageRange::english();
        for tag in ["en", "EN", "en-GB", "en-us", "en-Latn-GB"] {
            assert!(english.matches_tag(tag), "{tag}");
        }
        for tag in ["eng", "e", "fr", "fr-en", "enx-GB"] {
            assert!(!english.matches_tag(tag), "{tag}");
        }
        assert!(english.selects(&oxrdf::Literal::new_simple_literal("plain")));
        assert!(english.selects(
            &oxrdf::Literal::new_language_tagged_literal_unchecked(
                "authorisation",
                "en-GB"
            )
        ));
        assert!(!english.selects(
            &oxrdf::Literal::new_language_tagged_literal_unchecked(
                "autorisation",
                "fr"
            )
        ));
    }

    #[test]
    fn ranges_are_validated_and_normalised() {
        assert_eq!(
            LanguageRange::try_new("EN-gb").unwrap().as_str(),
            "en-gb"
        );
        for invalid in ["", "en-", "-en", "en_GB", "toolongtag", "1n"] {
            assert!(
                LanguageRange::try_new(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
