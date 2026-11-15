//! The `ga:normativeKeywordStyle` token: how declared BCP 14 keywords look.
use anyhow::{Result, anyhow};

/// A closed presentation for normative keywords, shared by HTML and PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeywordStyle {
    /// Upright small capitals, the W3C specification convention.
    SmallCaps,
    /// Bold italic.
    BoldItalic,
}

impl KeywordStyle {
    /// bounded: the fixed token vocabulary.
    const ALL: [Self; 2] = [Self::SmallCaps, Self::BoldItalic];

    /// The token's literal spelling, also the Typst theme value.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SmallCaps => "small-caps",
            Self::BoldItalic => "bold-italic",
        }
    }

    pub(crate) fn parse(token: &str, value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|style| style.as_str() == value)
            .ok_or_else(|| {
                anyhow!(
                    "theme token {token} must be one of {}",
                    Self::ALL.map(Self::as_str).join(", ")
                )
            })
    }

    /// The stylesheet rule for `.rfc2119`, in authored and native markup.
    pub(crate) fn css(self) -> &'static str {
        match self {
            Self::SmallCaps => {
                ".rfc2119{font-style:normal;font-weight:600;font-variant:\
                 small-caps;text-transform:lowercase;letter-spacing:.03em}"
            },
            Self::BoldItalic => ".rfc2119{font-style:italic;font-weight:700}",
        }
    }
}
