//! How each theme token applies to a website. Screen tokens set a colour
//! property of the website skin or a rule within it; print tokens describe
//! paper, pagination, covers, contents, annexes, notices, print type sizes and
//! PDF class diagrams, and have no website meaning. Every token of the theme
//! vocabulary is classified exactly once.
use {
    super::screen_value::{ScreenColour, align, font, length, weight},
    crate::publication::theme::boolean,
    anyhow::Result,
    oxrdf::Term,
};

/// What a screen token changes. Selectors are relative to the skin root.
#[derive(Clone, Copy)]
pub(super) enum Effect {
    /// A colour custom property of the skin root.
    Colour { property: &'static str, optional: bool },
    /// A length custom property of the skin root.
    Length { property: &'static str },
    /// The font family of the selected elements.
    Font { selector: &'static str, fallback: &'static str },
    /// The font weight of the selected headings.
    Weight { selector: &'static str },
    /// The horizontal alignment of the selected captions.
    Align { selector: &'static str },
    /// A declaration applied when the token is true, another when false.
    Switch {
        selector: &'static str,
        on:       &'static str,
        off:      &'static str,
    },
    /// Use the publisher's `ga:accentColor` as the accent.
    AuthorityAccent,
    /// Show the publication status in a vertical band.
    StatusBand,
    /// Applied by the conformance keyword stylesheet of every renderer.
    KeywordStyle,
}

pub(super) struct ScreenToken {
    pub name:   &'static str,
    pub effect: Effect,
}

const fn colour(name: &'static str, property: &'static str) -> ScreenToken {
    ScreenToken {
        name,
        effect: Effect::Colour { property, optional: false },
    }
}

const fn fill(name: &'static str, property: &'static str) -> ScreenToken {
    ScreenToken {
        name,
        effect: Effect::Colour { property, optional: true },
    }
}

const fn switch(
    name: &'static str,
    selector: &'static str,
    on: &'static str,
    off: &'static str,
) -> ScreenToken {
    ScreenToken { name, effect: Effect::Switch { selector, on, off } }
}

const PROSE_LINKS: &str = "article :is(p,li,td,th,dd)>a";
const CAPTIONS: &str = ":is(caption,figcaption)";
const CELLS: &str = "article :is(th,td)";

/// bounded: the fixed screen token vocabulary.
pub(super) const SCREEN_TOKENS: &[ScreenToken] = &[
    colour("accent", "--accent"),
    ScreenToken {
        name:   "authorityAccent",
        effect: Effect::AuthorityAccent,
    },
    ScreenToken { name: "statusRibbon", effect: Effect::StatusBand },
    colour("inkColor", "--ink"),
    colour("mutedColor", "--muted"),
    colour("ruleColor", "--line"),
    colour("panelColor", "--panel"),
    colour("headingColor", "--heading"),
    colour("linkColor", "--link"),
    switch(
        "linkUnderline",
        PROSE_LINKS,
        "text-decoration:underline",
        "text-decoration:none",
    ),
    ScreenToken {
        name:   "bodyFont",
        effect: Effect::Font { selector: "", fallback: "sans-serif" },
    },
    switch(
        "justify",
        "article p",
        "text-align:justify",
        "text-align:start",
    ),
    switch(
        "hyphenate",
        "article",
        "hyphens:auto",
        "hyphens:manual",
    ),
    ScreenToken {
        name:   "headingFont",
        effect: Effect::Font {
            selector: ":is(h1,h2,h3,h4,h5,h6)",
            fallback: "sans-serif",
        },
    },
    ScreenToken {
        name:   "heading1Weight",
        effect: Effect::Weight { selector: "article h1" },
    },
    ScreenToken {
        name:   "heading2Weight",
        effect: Effect::Weight { selector: "article h2" },
    },
    fill("tableHeaderFill", "--table-head"),
    colour("tableHeaderText", "--table-head-text"),
    switch(
        "tableGrid",
        CELLS,
        "border:1px solid var(--table-rule,var(--line))",
        "border-width:0 0 1px",
    ),
    colour("tableRuleColor", "--table-rule"),
    ScreenToken {
        name:   "captionFont",
        effect: Effect::Font { selector: CAPTIONS, fallback: "sans-serif" },
    },
    switch(
        "captionBold",
        CAPTIONS,
        "font-weight:600",
        "font-weight:400",
    ),
    switch(
        "tableCaptionAbove",
        "caption",
        "caption-side:top",
        "caption-side:bottom",
    ),
    ScreenToken {
        name:   "figureCaptionAlign",
        effect: Effect::Align { selector: "figcaption" },
    },
    ScreenToken {
        name:   "codeFont",
        effect: Effect::Font {
            selector: ":is(code,pre,kbd,samp)",
            fallback: "ui-monospace,monospace",
        },
    },
    fill("codeFill", "--code-fill"),
    fill("codeRuleColor", "--code-rule"),
    // Highlighted tokens carry their colour inline; plain code inherits.
    switch(
        "codeHighlight",
        "pre span[style]",
        "",
        "color:inherit!important",
    ),
    colour("quoteRuleColor", "--quote-rule"),
    switch(
        "tintedCallouts",
        "blockquote[class*=\"markdown-alert-\"]",
        "",
        "background:transparent",
    ),
    ScreenToken {
        name:   "normativeKeywordStyle",
        effect: Effect::KeywordStyle,
    },
    colour("footerColor", "--footer-text"),
    ScreenToken {
        name:   "contentWidth",
        effect: Effect::Length { property: "--content-width" },
    },
];

/// bounded: the fixed print-only token vocabulary, which the classification
/// test checks against the theme vocabulary.
#[cfg(test)]
const PRINT_TOKENS: &[&str] = &[
    "paper",
    "marginTop",
    "marginBottom",
    "marginLeft",
    "marginRight",
    "footerDescent",
    "coverBands",
    "runningHeader",
    "bodySize",
    "leading",
    "paragraphSpacing",
    "numberHeadings",
    "headingNumberGap",
    "chapterPageBreak",
    "heading1Size",
    "heading2Size",
    "heading3Size",
    "heading4Size",
    "tableTextSize",
    "captionSize",
    "tableNumberByChapter",
    "diagramFont",
    "diagramFocusFill",
    "diagramClassFill",
    "diagramImportedClassFill",
    "diagramExpressionFill",
    "diagramObjectPropertyFill",
    "diagramDatatypePropertyFill",
    "diagramPropertyFill",
    "diagramDatatypeFill",
    "diagramLiteralFill",
    "codeSize",
    "codeInset",
    "quoteTextSize",
    "quoteInsetX",
    "quoteInsetY",
    "footerFont",
    "footerSize",
    "footerRule",
    "footerShowsText",
    "pageNumberColor",
    "pageNumberBold",
    "pageOfTotal",
    "outerPageNumber",
    "frontNumbering",
    "bodyNumbering",
    "tocTitle",
    "tocTitleAlign",
    "tocTitleSize",
    "tocTitleFont",
    "tocFont",
    "tocEntrySize",
    "contentsDepth",
    "annexLabel",
    "annexNormativity",
    "annexCentered",
    "annexRectoStart",
    "blankPageText",
    "noticeFont",
    "noticeSize",
    "noticeJustify",
    "noticeTitleFont",
    "noticeTitleSize",
    "noticeTitleRule",
    "noticeHeadingFont",
    "noticeHeadingSize",
    "noticeHeadingAlign",
    "noticeHeadingUppercase",
    "coverFont",
    "specificationCover",
];

/// One validated declaration of a screen token.
pub(super) enum Declaration {
    /// A custom property of the skin root.
    Property(&'static str, String),
    /// A rule for a selector within the skin root.
    Rule(&'static str, String),
    /// Nothing to declare: the skin default applies.
    None,
}

impl ScreenToken {
    pub fn declaration(&self, value: &Term) -> Result<Declaration> {
        let name = self.name;
        Ok(match self.effect {
            Effect::Colour { property, optional } => {
                Declaration::Property(
                    property,
                    ScreenColour::try_new(name, value, optional)?.css().into(),
                )
            },
            Effect::Length { property } => {
                Declaration::Property(property, length(name, value)?.into())
            },
            Effect::Font { selector, fallback } => {
                Declaration::Rule(
                    selector,
                    format!("font-family:{}", font(name, value, fallback)?),
                )
            },
            Effect::Weight { selector } => {
                Declaration::Rule(
                    selector,
                    format!("font-weight:{}", weight(name, value)?),
                )
            },
            Effect::Align { selector } => {
                Declaration::Rule(
                    selector,
                    format!("text-align:{}", align(name, value)?),
                )
            },
            Effect::Switch { selector, on, off } => {
                match if boolean(name, value)? { on } else { off } {
                    "" => Declaration::None,
                    declaration => {
                        Declaration::Rule(selector, declaration.into())
                    },
                }
            },
            Effect::AuthorityAccent |
            Effect::StatusBand |
            Effect::KeywordStyle => {
                boolean_or_keyword(name, value, self.effect)?;
                Declaration::None
            },
        })
    }
}

/// Validates the tokens other website parts apply.
fn boolean_or_keyword(name: &str, value: &Term, effect: Effect) -> Result<()> {
    match effect {
        Effect::KeywordStyle => Ok(()),
        _ => boolean(name, value).map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{PRINT_TOKENS, SCREEN_TOKENS},
        crate::publication::theme::TOKENS,
    };

    #[test]
    fn every_theme_token_is_classified_once() {
        for token in TOKENS {
            let screen = SCREEN_TOKENS
                .iter()
                .filter(|screen| screen.name == token.name)
                .count();
            let print = PRINT_TOKENS
                .iter()
                .filter(|name| **name == token.name)
                .count();
            assert_eq!(
                screen + print,
                1,
                "ga:{} must be either a screen or a print token",
                token.name
            );
        }
        for name in SCREEN_TOKENS
            .iter()
            .map(|screen| screen.name)
            .chain(PRINT_TOKENS.iter().copied())
        {
            assert!(
                TOKENS.iter().any(|token| token.name == name),
                "ga:{name} is not a theme token"
            );
        }
    }
}
