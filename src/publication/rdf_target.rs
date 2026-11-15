//! RDF renderer targets: one file per declared RDF format, built from the
//! collected text of the target's RDF sources. Format media types and file
//! extensions come from the knowledge graph; renderers never reread sources.
mod format;
mod merged;
mod names;
mod negotiation;
mod source_text;
mod target;
pub(crate) use {
    names::{MediaType, NegotiatedPath},
    source_text::SourceText,
    target::{PublishedRdfFile, RdfTarget},
};
