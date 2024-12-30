use {
    super::{
        JekyllMarkdownDocumentorImpl,
        MarkdownDocumentorImpl,
        OWLClassDocumentorImpl,
        OWLOntologyDocumentorImpl,
    },
    crate::{
        model::DocumentationModel,
        source::FileSourceImplementor,
        store::LoaderStore,
        util::FileTypeSliceStatic,
    },
    async_trait::async_trait,
    std::{fmt::Debug, path::Path},
};

/// A trait for documentors that can document asynchronously.
#[async_trait]
pub trait Documentor {
    /// Returns the file types that this documentor can process. If
    /// any since some Documentor implementations can only work with
    /// data from non-file sources such as a database.
    fn file_types(&self) -> FileTypeSliceStatic { &[] }

    /// Return the optional primary file name that this documentor
    /// cares about.
    fn file_name(&self) -> Option<&Path>;

    /// Generates the documentatable items into the
    /// `DocumentationModel`. This is the only step where a
    /// Documentor is allowed to mutate the `DocumentationModel`.
    async fn generate(&self) -> anyhow::Result<()>;
}

pub trait DocumentorCreator: Sized {
    /// Creates a new `Documentor.
    ///
    /// # Arguments
    ///
    /// * `file_source` - The optional file source that contains the
    ///   data that this documentor will document.
    /// * `file_name` - The most relevant file name of the file that
    ///   this
    /// documentor will document, if any.
    /// * `loader_store` - The loader store that contains the data
    ///   that this
    /// documentor will document.
    /// * `doc_model` - The documentation model that contains the data
    ///   that this
    /// documentor will document.
    fn new(
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> Self
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentorVariant {
    OWLOntology,
    OWLClass,
    Markdown,
    JekyllMarkdown,
}

/// An enum that holds all the possible documentor implementations.
/// This enum primarily exists to avoid having to use trait objects.
/// Obviously keep the members of this enum in sync with the
/// DocumentorVariant enum.
#[derive(Debug, Clone)]
pub enum DocumentorImplementor {
    OWLOntologyDocumentor(OWLOntologyDocumentorImpl),
    OWLClassDocumentor(OWLClassDocumentorImpl),
    MarkdownDocumentor(MarkdownDocumentorImpl),
    JekyllMarkdownDocumentor(JekyllMarkdownDocumentorImpl),
}

impl DocumentorImplementor {
    pub fn new(
        variant: DocumentorVariant,
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> Self {
        match variant {
            DocumentorVariant::OWLOntology => {
                Self::OWLOntologyDocumentor(
                    OWLOntologyDocumentorImpl::new(
                        file_source,
                        file_name,
                        loader_store,
                        doc_model,
                    ),
                )
            },
            DocumentorVariant::OWLClass => {
                Self::OWLClassDocumentor(OWLClassDocumentorImpl::new(
                    file_source,
                    file_name,
                    loader_store,
                    doc_model,
                ))
            },
            DocumentorVariant::Markdown => {
                Self::MarkdownDocumentor(MarkdownDocumentorImpl::new(
                    file_source,
                    file_name,
                    loader_store,
                    doc_model,
                ))
            },
            DocumentorVariant::JekyllMarkdown => {
                Self::JekyllMarkdownDocumentor(
                    JekyllMarkdownDocumentorImpl::new(
                        file_source,
                        file_name,
                        loader_store,
                        doc_model,
                    ),
                )
            },
        }
    }
}

#[async_trait]
impl Documentor for DocumentorImplementor {
    async fn generate(&self) -> anyhow::Result<()> {
        match self {
            DocumentorImplementor::OWLOntologyDocumentor(
                documentor,
            ) => documentor.generate().await,
            DocumentorImplementor::OWLClassDocumentor(documentor) => {
                documentor.generate().await
            },
            DocumentorImplementor::MarkdownDocumentor(documentor) => {
                documentor.generate().await
            },
            DocumentorImplementor::JekyllMarkdownDocumentor(
                documentor,
            ) => documentor.generate().await,
        }
    }

    fn file_name(&self) -> Option<&Path> {
        match self {
            DocumentorImplementor::OWLOntologyDocumentor(
                documentor,
            ) => documentor.file_name(),
            DocumentorImplementor::OWLClassDocumentor(documentor) => {
                documentor.file_name()
            },
            DocumentorImplementor::MarkdownDocumentor(documentor) => {
                documentor.file_name()
            },
            DocumentorImplementor::JekyllMarkdownDocumentor(
                documentor,
            ) => documentor.file_name(),
        }
    }
}
