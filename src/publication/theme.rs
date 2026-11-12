//! House styles are RDF resources identified by IRIs. A profile selects one
//! with `ga:theme`; a theme can refine another with `ga:baseTheme`, and every
//! chain ends at a built-in catalogue theme. Tokens are declarative values that
//! the renderer maps to Typst; supplied text is never evaluated as source.
mod builtin;
mod cover;
mod lineage;
mod pdf;
mod tokens;
mod values;

#[cfg(test)]
pub(crate) use pdf::catalogue_typst;
pub(crate) use {
    builtin::Builtin,
    cover::{CoverBlock, CoverElement, DateFormat},
    lineage::Lineage,
    pdf::PdfTheme,
};
