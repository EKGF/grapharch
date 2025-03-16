use {
    crate::{
        documentor::{Documentor, DocumentorCreator},
        model::{Book, Buildable, Element, Model},
        source::{FileSource, FileSourceImplementor},
        store::LoaderStore,
        util::{FileType, FileTypeSliceStatic},
    },
    async_trait::async_trait,
    serde::Deserialize,
    std::{
        path::{Path, PathBuf},
        sync::{Arc, LazyLock},
    },
    tracing::info,
};

static JEKYLL_MARKDOWN_DOCUMENTOR_FILE_TYPES: LazyLock<
    &'static [&'static FileType],
> = LazyLock::new(|| {
    let file_types = vec![&FileType::JekyllConfig, &FileType::Markdown];
    Box::leak(Box::new(file_types))
});

/// Struct to deserialize the Jekyll _config.yml file.
#[derive(Deserialize, Debug)]
struct JekyllConfig {
    #[allow(unused)]
    title_separator: Option<String>,
    #[allow(unused)]
    repository:      Option<String>,
    #[allow(unused)]
    url:             Option<String>,
    #[allow(unused)]
    #[serde(default)]
    author:          AuthorField,
}

/// Author field can be either a string or a map with a name field
#[derive(Deserialize, Debug, Default)]
#[serde(untagged)]
enum AuthorField {
    #[default]
    None,
    String(String),
    Map {
        name: String,
    },
}

impl AuthorField {
    fn as_string(&self) -> Option<String> {
        match self {
            AuthorField::None => None,
            AuthorField::String(s) => Some(s.clone()),
            AuthorField::Map { name } => Some(name.clone()),
        }
    }
}

/// A documentor for Markdown files in a directory that is managed by
/// a Jekyll config file (_config.yml) (see
/// `FILE_TYPE_JEKYLL_CONFIG`).
#[derive(Debug, Clone)]
pub struct JekyllMarkdownDocumentorImpl {
    file_source:  FileSourceImplementor,
    #[allow(unused)]
    file_name:    Option<PathBuf>,
    #[allow(unused)]
    loader_store: LoaderStore,
    /// The given target documentation model that the
    /// MarkdownDocumentor will add its documentation to.
    #[allow(unused)]
    doc_model:    Arc<Model>,
}

impl DocumentorCreator for JekyllMarkdownDocumentorImpl {
    fn new(
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: Arc<Model>,
    ) -> Self {
        Self {
            file_source: file_source.unwrap(),
            file_name: file_name.map(|f| f.to_path_buf()),
            loader_store,
            doc_model,
        }
    }
}

#[async_trait]
impl Documentor for JekyllMarkdownDocumentorImpl {
    fn file_types(&self) -> FileTypeSliceStatic {
        *JEKYLL_MARKDOWN_DOCUMENTOR_FILE_TYPES
    }

    fn file_name(&self) -> Option<&Path> { self.file_name.as_deref() }

    async fn generate(&self) -> anyhow::Result<()> {
        // Get the content of the _config.yml file that's expected to
        // be in the root.
        let config: JekyllConfig = self.get_config().await?;

        // Set the BookBuilder attributes based on the config
        Book::builder_in_model::<Book>(&self.doc_model)?
            .title(Some("Markdown Documentation".to_string()))
            .repository(config.repository)
            .url(config.url)
            .author(config.author.as_string())
            .build()?;

        // Process the rest
        self.process().await?;

        Ok(())
    }
}

impl JekyllMarkdownDocumentorImpl {
    async fn get_config(&self) -> anyhow::Result<JekyllConfig> {
        let config_file = self
            .file_source
            .content_of(self.file_name.as_ref().unwrap())
            .await?;
        Ok(serde_yaml::from_str(&config_file)?)
    }

    /// Processes the provided markdown files.
    pub async fn process(&self) -> anyhow::Result<()> {
        // Scan again for only the markdown files.
        let markdown_files =
            self.file_source.scan(&[&FileType::Markdown]).await?;

        for file_path in markdown_files {
            let content = self.file_source.content_of(&file_path).await?;
            self.process_markdown(&content).await?;
        }
        Ok(())
    }

    /// Processes the content of a markdown file and adds it to the
    /// documentation model.
    async fn process_markdown(&self, content: &str) -> anyhow::Result<()> {
        // Log the length of the markdown content being processed
        info!(
            "Processing markdown content of length: {}",
            content.len()
        );

        // Example: Add a section to the documentation model
        // self.doc_model.add_section().await?;

        Ok(())
    }
}
