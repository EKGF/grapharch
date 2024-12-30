use {
    super::{MarkdownLoader, RDFLoader},
    crate::{
        documentor::DocumentorImplementor,
        model::DocumentationModel,
        source::FileSourceImplementor,
        store::LoaderStore,
        util::FileTypeSliceStatic,
    },
    async_trait::async_trait,
    std::path::PathBuf,
};

/// A trait for documentors that can load information from
/// a given source asynchronously.
#[async_trait]
pub trait Loader: std::fmt::Debug + std::fmt::Display {
    /// Returns the file types that this loader can process.
    fn file_types(&self) -> FileTypeSliceStatic;

    /// Returns true if the given file name matches any of the file
    /// types that the loader can process.
    fn is_applicable(&self, file_name: &PathBuf) -> bool {
        self.file_types().iter().any(|t| t.is_of_type(file_name))
    }

    fn applicable_files<'a>(
        &self,
        file_names: &'a Vec<&'a PathBuf>,
    ) -> Vec<&'a PathBuf> {
        file_names
            .iter()
            .filter(|file_name| self.is_applicable(file_name))
            .map(|file_name| *file_name)
            .collect()
    }

    /// Asynchronously loads the given files into the given
    /// LoaderStore. They are files that have types that are in
    /// the `FileType`s returned by `file_types`.
    /// Then return a collection of documentors that were created
    /// but have not been executed yet (i.e. they have not been
    /// called with `generate`).
    async fn load_files(
        &self,
        file_source: &FileSourceImplementor,
        file_names: &Vec<&PathBuf>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> anyhow::Result<Vec<DocumentorImplementor>>;
}

#[derive(Debug)]
pub enum LoaderImplementor {
    MarkdownLoader(MarkdownLoader),
    RDFLoader(RDFLoader),
}

#[async_trait]
impl Loader for LoaderImplementor {
    fn file_types(&self) -> FileTypeSliceStatic {
        match self {
            LoaderImplementor::MarkdownLoader(loader) => {
                loader.file_types()
            },
            LoaderImplementor::RDFLoader(loader) => {
                loader.file_types()
            },
        }
    }

    async fn load_files(
        &self,
        file_source: &FileSourceImplementor,
        file_names: &Vec<&PathBuf>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> anyhow::Result<Vec<DocumentorImplementor>> {
        let applicable_file_names = self.applicable_files(file_names);
        tracing::info!(
            "The {:} will load {} applicable files into the \
             loader-store",
            self,
            applicable_file_names.len()
        );
        match self {
            LoaderImplementor::MarkdownLoader(loader) => {
                loader
                    .load_files(
                        file_source,
                        &applicable_file_names,
                        loader_store,
                        doc_model,
                    )
                    .await
            },
            LoaderImplementor::RDFLoader(loader) => {
                loader
                    .load_files(
                        file_source,
                        &applicable_file_names,
                        loader_store,
                        doc_model,
                    )
                    .await
            },
        }
    }
}

impl std::fmt::Display for LoaderImplementor {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            LoaderImplementor::MarkdownLoader(loader) => {
                loader.fmt(f)
            },
            LoaderImplementor::RDFLoader(loader) => loader.fmt(f),
        }
    }
}
