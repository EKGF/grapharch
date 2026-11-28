//! The inert SVG preamble tokens that common exporters write: the standard
//! W3C SVG public document type and PlantUML's source annotation. Anything
//! else in a document type or processing instruction stays rejected.
use {
    anyhow::{Result, bail},
    xmlparser::{ExternalId, Token},
};

/// The W3C SVG public identifiers and their canonical DTD locations.
const STANDARD_DOCTYPES: [(&str, &str); 2] = [
    (
        "-//W3C//DTD SVG 1.0//EN",
        "http://www.w3.org/TR/2001/REC-SVG-20010904/DTD/svg10.dtd",
    ),
    (
        "-//W3C//DTD SVG 1.1//EN",
        "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd",
    ),
];

/// The processing-instruction target PlantUML writes into generated SVG.
const PLANTUML_ANNOTATION: &str = "plantuml";

/// One SVG token checked against the inert preamble allowlist.
pub(crate) struct SvgToken<'a>(pub(crate) Token<'a>);
impl SvgToken<'_> {
    /// Fail on a document type, entity or processing instruction outside the
    /// allowlist. Bytes are never rewritten and no DTD is fetched.
    pub(crate) fn check(&self) -> Result<()> {
        match &self.0 {
            Token::EmptyDtd {
                name,
                external_id: Some(ExternalId::Public(public, system)),
                ..
            } if name.as_str() == "svg" &&
                STANDARD_DOCTYPES.iter().any(|(id, dtd)| {
                    public.as_str() == *id && system.as_str() == *dtd
                }) =>
            {
                Ok(())
            },
            Token::ProcessingInstruction { target, content, .. }
                if target.as_str() == PLANTUML_ANNOTATION &&
                    content.is_some_and(|v| is_version(v.as_str())) =>
            {
                Ok(())
            },
            Token::DtdStart { .. } |
            Token::EmptyDtd { .. } |
            Token::EntityDeclaration { .. } |
            Token::DtdEnd { .. } |
            Token::ProcessingInstruction { .. } => {
                bail!(
                    "SVG document types, custom entities and processing \
                     instructions are unsupported"
                )
            },
            _ => Ok(()),
        }
    }

    /// Whether this checked token is the document type declaration.
    pub(crate) fn is_doctype(&self) -> bool {
        matches!(self.0, Token::EmptyDtd { .. })
    }

    /// Whether a parsed processing instruction is PlantUML's annotation; its
    /// content was checked by [`Self::check`] during tokenization.
    pub(crate) fn is_inert_instruction(target: &str) -> bool {
        target == PLANTUML_ANNOTATION
    }
}

/// PlantUML writes its release, such as `1.2025.0`, as the annotation.
fn is_version(value: &str) -> bool {
    !value.is_empty() &&
        value.len() <= 64 &&
        value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
}
