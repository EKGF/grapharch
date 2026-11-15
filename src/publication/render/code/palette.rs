//! Lexical token classes shared by every highlighted language.
//!
//! Each class names the TextMate scopes it colours, so SPARQL, Turtle and
//! JSON(-LD) present the same construct with the same colour in HTML and PDF.
use syntect::highlighting::{Color, ScopeSelectors, StyleModifier, ThemeItem};

/// A lexical role with one colour across languages and output formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TokenClass {
    /// Comments.
    Comment,
    /// Keywords, including `@prefix`, `PREFIX`, `@base`, `BASE` and `a`.
    Keyword,
    /// String literals and IRI references.
    Text,
    /// Numeric literals.
    Number,
    /// `true`, `false` and other language constants.
    Constant,
    /// The prefix of a prefixed name, including its colon.
    Namespace,
    /// The local part of a prefixed name, SPARQL variables and JSON keys.
    Name,
    /// Language tags and datatype markers that annotate a literal.
    LiteralAnnotation,
    /// Blank node labels.
    BlankNode,
    /// Statement and collection punctuation.
    Punctuation,
}

impl TokenClass {
    /// Declaration order is precedence order for equally specific scopes.
    pub(super) const ALL: [Self; 10] = [
        Self::Punctuation,
        Self::Text,
        Self::Number,
        Self::Constant,
        Self::Keyword,
        Self::Namespace,
        Self::Name,
        Self::LiteralAnnotation,
        Self::BlankNode,
        Self::Comment,
    ];

    pub(super) const fn color(self) -> Color {
        let (r, g, b) = match self {
            Self::Comment => (91, 101, 117),
            Self::Keyword => (161, 42, 54),
            Self::Text => (20, 107, 54),
            Self::Number => (154, 77, 0),
            Self::Constant | Self::Namespace => (0, 90, 156),
            Self::Name => (104, 54, 181),
            Self::LiteralAnnotation => (0, 110, 120),
            Self::BlankNode => (150, 60, 20),
            Self::Punctuation => (70, 84, 102),
        };
        Color { r, g, b, a: 255 }
    }

    const fn scopes(self) -> &'static str {
        match self {
            Self::Comment => {
                "comment, comment.line, comment.block, comment \
                 meta.whitespace, punctuation.definition.comment"
            },
            Self::Keyword => {
                "keyword, storage.type, punctuation.definition.storage, \
                 support.constant.predicate.a"
            },
            Self::Text => {
                "string, string.quoted, source.ttl string, source.rq string, \
                 source.json string, source.json meta.mapping.value.json \
                 string.quoted.double.json, punctuation.definition.string, \
                 punctuation.literal, string.unquoted.iri, \
                 punctuation.definition.iri"
            },
            Self::Number => {
                "constant.numeric, meta.numeric, source.json constant.numeric, \
                 punctuation.decimal, keyword.operator.arithmetic.sign"
            },
            Self::Constant => {
                "constant.language, constant.language.boolean, source.json \
                 constant.language"
            },
            Self::Namespace => {
                "variable.other.readwrite.prefixed-name, \
                 punctuation.separator.prefixed-name"
            },
            Self::Name => {
                // More specific than Typst's own JSON key selectors, which
                // win ties and would otherwise outrank this palette.
                "variable.other.member.prefixed-name, source.rq variable, \
                 meta.mapping.key.json string, source.json \
                 meta.mapping.key.json string.quoted.double.json"
            },
            Self::LiteralAnnotation => {
                "string.unquoted.language-tag, \
                 punctuation.separator.language-tag, \
                 punctuation.separator.datatype"
            },
            Self::BlankNode => {
                "variable.other.readwrite.blank-node, \
                 variable.other.member.blank-node"
            },
            Self::Punctuation => {
                "punctuation.terminator, punctuation.separator, \
                 punctuation.section, \
                 punctuation.definition.blank-node-property-list, \
                 punctuation.definition.anonymous-blank-node, \
                 punctuation.definition.triple-x"
            },
        }
    }

    pub(super) fn theme_item(self) -> ThemeItem {
        ThemeItem {
            scope: self
                .scopes()
                .parse::<ScopeSelectors>()
                .expect("bundled token class scopes"),
            style: StyleModifier {
                foreground: Some(self.color()),
                ..StyleModifier::default()
            },
        }
    }
}
