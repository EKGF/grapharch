mod markdown;
mod owl;
mod this;

pub use {
    markdown::{JekyllMarkdownDocumentorImpl, MarkdownDocumentorImpl},
    owl::{OWLClassDocumentorImpl, OWLOntologyDocumentorImpl},
    this::{
        Documentor,
        DocumentorCreator,
        DocumentorImplementor,
        DocumentorVariant,
    },
};
