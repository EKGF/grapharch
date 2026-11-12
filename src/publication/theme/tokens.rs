//! The declarative PDF token vocabulary. Every token is a `ga:` datatype
//! property on a `ga:Theme`; its Typst key is the snake-case local name.
use super::values::Kind::{self, *};

/// One supported theme token.
pub(crate) struct Token {
    pub name:     &'static str,
    pub kind:     Kind,
    pub required: bool,
}

const fn required(name: &'static str, kind: Kind) -> Token {
    Token { name, kind, required: true }
}

const fn optional(name: &'static str, kind: Kind) -> Token {
    Token { name, kind, required: false }
}

/// bounded: the fixed token vocabulary, independent of corpus size.
pub(crate) const TOKENS: &[Token] = &[
    // Page geometry and page decoration.
    required("paper", Paper),
    required("marginTop", Length),
    required("marginBottom", Length),
    required("marginLeft", Length),
    required("marginRight", Length),
    required("footerDescent", Relative),
    required("coverBands", Bool),
    required("statusRibbon", Bool),
    required("runningHeader", Bool),
    // Colors. The accent is a #RRGGBB value; other colors may refer to it.
    required("accent", Color),
    required("authorityAccent", Bool),
    required("inkColor", Color),
    required("mutedColor", Color),
    required("ruleColor", Color),
    required("panelColor", Color),
    required("headingColor", Color),
    required("linkColor", Color),
    required("linkUnderline", Bool),
    // Body text.
    required("bodyFont", Font),
    required("bodySize", Length),
    required("justify", Bool),
    required("hyphenate", Bool),
    required("leading", Length),
    required("paragraphSpacing", Length),
    // Headings.
    required("headingFont", Font),
    required("numberHeadings", Bool),
    optional("headingNumberGap", Length),
    required("chapterPageBreak", Bool),
    required("heading1Size", Length),
    required("heading1Weight", Weight),
    required("heading2Size", Length),
    required("heading2Weight", Weight),
    required("heading3Size", Length),
    required("heading4Size", Length),
    // Tables and captions.
    required("tableTextSize", Length),
    required("tableHeaderFill", OptionalColor),
    required("tableHeaderText", Color),
    required("tableGrid", Bool),
    required("tableRuleColor", Color),
    required("captionFont", Font),
    required("captionSize", Length),
    required("captionBold", Bool),
    required("tableCaptionAbove", Bool),
    // Code and quotations.
    required("codeFont", Font),
    required("codeSize", Length),
    required("codeFill", OptionalColor),
    required("codeRuleColor", OptionalColor),
    required("codeHighlight", Bool),
    required("codeInset", Length),
    required("quoteRuleColor", Color),
    required("quoteTextSize", Length),
    required("quoteInsetX", Length),
    required("quoteInsetY", Length),
    required("tintedCallouts", Bool),
    // Footer and page numbers.
    required("footerFont", Font),
    required("footerSize", Length),
    required("footerColor", Color),
    required("footerRule", Bool),
    required("footerShowsText", Bool),
    required("pageNumberColor", Color),
    required("pageNumberBold", Bool),
    required("pageOfTotal", Bool),
    required("outerPageNumber", Bool),
    optional("frontNumbering", Numbering),
    optional("bodyNumbering", Numbering),
    // Table of contents.
    required("tocTitle", Text),
    required("tocTitleAlign", Align),
    required("tocTitleSize", Length),
    required("tocTitleFont", Font),
    required("tocFont", Font),
    required("tocEntrySize", Length),
    required("contentsDepth", Depth),
    // Annexes.
    optional("annexLabel", Pattern("{letter}")),
    required("annexNormativity", Bool),
    required("annexCentered", Bool),
    required("annexRectoStart", Bool),
    optional("blankPageText", Text),
    // Front-matter notices.
    required("noticeFont", Font),
    required("noticeSize", Length),
    required("noticeJustify", Bool),
    required("noticeTitleFont", Font),
    required("noticeTitleSize", Length),
    required("noticeTitleRule", Bool),
    required("noticeHeadingFont", Font),
    required("noticeHeadingSize", Length),
    required("noticeHeadingAlign", Align),
    required("noticeHeadingUppercase", Bool),
    // Cover.
    required("coverFont", Font),
    required("specificationCover", Bool),
];

/// Typst dictionary key for a token: `marginTop` becomes `margin_top`.
pub(crate) fn key(name: &str) -> String {
    let mut key = String::with_capacity(name.len() + 4);
    for character in name.chars() {
        if character.is_ascii_uppercase() || character.is_ascii_digit() {
            key.push('_');
        }
        key.push(character.to_ascii_lowercase());
    }
    key
}

pub(crate) fn token(name: &str) -> Option<&'static Token> {
    TOKENS.iter().find(|token| token.name == name)
}
