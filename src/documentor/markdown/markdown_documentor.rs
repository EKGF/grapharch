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

static MARKDOWN_DOCUMENTOR_FILE_TYPES: LazyLock<FileTypeSliceStatic> =
    LazyLock::new(|| {
        let file_types = vec![&FileType::Markdown];
        Box::leak(Box::new(file_types))
    });

/// A documentor for Markdown files.
#[derive(Debug, Clone)]
pub struct MarkdownDocumentorImpl {
    #[allow(unused)]
    file_source:  FileSourceImplementor,
    #[allow(unused)]
    file_name:    Option<PathBuf>,
    #[allow(unused)]
    loader_store: LoaderStore,
    /// The given target documentation model that the
    /// MarkdownDocumentor will add its documentation to.
    #[allow(unused)]
    doc_model:    DocumentationModel,
}

#[async_trait]
impl Documentor for MarkdownDocumentorImpl {
    fn file_types(&self) -> FileTypeSliceStatic {
        *MARKDOWN_DOCUMENTOR_FILE_TYPES
    }

    fn file_name(&self) -> Option<&Path> {
        self.file_name.as_ref().map(|f| f.as_path())
    }

    async fn generate(&self) -> anyhow::Result<()> { todo!() }
}

impl DocumentorCreator for MarkdownDocumentorImpl {
    fn new(
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> Self {
        Self {
            file_source: file_source.unwrap(),
            file_name: file_name.map(|f| f.to_path_buf()),
            loader_store,
            doc_model,
        }
    }
}