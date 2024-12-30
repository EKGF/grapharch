mod documentor;
mod markdown;
mod owl;

pub use {
    documentor::{
        Documentor,
        DocumentorCreator,
        DocumentorImplementor,
        DocumentorVariant,
    },
    markdown::{
        JekyllMarkdownDocumentorImpl,
        MarkdownDocumentorImpl,
    },
    owl::{OWLClassDocumentorImpl, OWLOntologyDocumentorImpl},
};
