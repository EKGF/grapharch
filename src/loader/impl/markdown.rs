use {
    super::super::Loader,
    crate::{
        documentor::{DocumentorImplementor, DocumentorVariant},
        model::DocumentationModel,
        source::{FileSource, FileSourceImplementor},
        store::LoaderStore,
        util::{FileType, FileTypeSliceStatic, relative_path},
    },
    async_trait::async_trait,
    std::path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct MarkdownLoader {}

#[async_trait]
impl Loader for MarkdownLoader {
    /// Returns the file types that this loader can process.
    /// This loader can process _config.yml files and markdown files.
    fn file_types(&self) -> FileTypeSliceStatic {
        &[&FileType::JekyllConfig, &FileType::Markdown]
    }

    async fn load_files(
        &self,
        file_source: &FileSourceImplementor,
        file_names: &Vec<&PathBuf>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> anyhow::Result<Vec<DocumentorImplementor>> {
        let documentors = futures::future::try_join_all(
            file_names.into_iter().map(|file_name| {
                self.load_file(
                    file_source,
                    file_name.as_path(),
                    loader_store.clone(),
                    doc_model.clone(),
                )
            }),
        )
        .await?
        .into_iter()
        .flatten()
        .collect();

        Ok(documentors)
    }
}

impl std::fmt::Display for MarkdownLoader {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Markdown-loader")
    }
}

impl MarkdownLoader {
    // fn is_jekyll_site(file_names: &Vec<PathBuf>) -> bool {
    //     file_names.contains(&
    // PathBuf::from(JEKYLL_CONFIG_FILE_NAME)) }

    async fn load_file(
        &self,
        file_source: &FileSourceImplementor,
        file_name: &Path,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> anyhow::Result<Vec<DocumentorImplementor>> {
        tracing::info!(
            "Loading Markdown file {:}",
            relative_path(
                file_name,
                file_source.root_path().unwrap()
            )
            .display()
        );
        let mut documentors = Vec::new();

        match FileType::from_path(file_name) {
            Some(FileType::JekyllConfig) => {
                let documentor = DocumentorImplementor::new(
                    DocumentorVariant::JekyllMarkdown,
                    Some(file_source.clone()),
                    Some(file_name),
                    loader_store.clone(),
                    doc_model.clone(),
                );
                documentors.push(documentor);
            },
            Some(FileType::Markdown) => {
                let documentor = DocumentorImplementor::new(
                    DocumentorVariant::Markdown,
                    Some(file_source.clone()),
                    Some(file_name),
                    loader_store.clone(),
                    doc_model.clone(),
                );
                documentors.push(documentor);
            },
            _ => {
                tracing::error!(
                    "Unknown type of file: {}",
                    file_name.display()
                );
            },
        }

        Ok(documentors)
    }
}
