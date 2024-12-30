use {
    crate::{
        documentor::{Documentor, DocumentorCreator},
        model::DocumentationModel,
        source::FileSourceImplementor,
        store::LoaderStore,
        util::{FileType, FileTypeSliceStatic},
    },
    async_trait::async_trait,
    std::{
        path::{Path, PathBuf},
        sync::LazyLock,
    },
};

static OWL_CLASS_DOCUMENTOR_FILE_TYPES: LazyLock<
    &'static [&'static FileType],
> = LazyLock::new(|| {
    let file_types = vec![
        &FileType::RdfXml,
        &FileType::NTriples,
        &FileType::JSONLD,
        &FileType::Turtle,
        &FileType::NQuads,
        &FileType::N3,
        &FileType::TriG,
    ];
    Box::leak(Box::new(file_types))
});

/// A documentor for OWL classes.
#[derive(Debug, Clone)]
pub struct OWLClassDocumentorImpl {
    #[allow(unused)]
    file_source:  Option<FileSourceImplementor>,
    #[allow(unused)]
    file_name:    Option<PathBuf>,
    #[allow(unused)]
    loader_store: LoaderStore,
    /// The given target documentation model that the
    /// OWLClassDocumentor will add its documentation to.
    #[allow(unused)]
    doc_model:    DocumentationModel,
}

impl DocumentorCreator for OWLClassDocumentorImpl {
    fn new(
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> Self {
        Self {
            file_source,
            file_name: file_name.map(|f| f.to_path_buf()),
            loader_store,
            doc_model,
        }
    }
}

#[async_trait]
impl Documentor for OWLClassDocumentorImpl {
    fn file_types(&self) -> FileTypeSliceStatic {
        *OWL_CLASS_DOCUMENTOR_FILE_TYPES
    }

    async fn generate(&self) -> anyhow::Result<()> { todo!() }

    fn file_name(&self) -> Option<&Path> { self.file_name.as_deref() }
}
