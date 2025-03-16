use {
    crate::{
        documentor::{Documentor, DocumentorCreator},
        model::{Book, Buildable, Chapter, Element, Model, Section},
        source::FileSourceImplementor,
        store::LoaderStore,
        util::{FileType, FileTypeSliceStatic},
    },
    async_trait::async_trait,
    futures::future::{Future, join_all},
    std::{
        path::{Path, PathBuf},
        sync::{Arc, LazyLock},
    },
    tracing::info,
};

static OWL_ONTOLOGY_DOCUMENTOR_FILE_TYPES: LazyLock<FileTypeSliceStatic> =
    LazyLock::new(|| {
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

/// A documentor for OWL ontologies.
#[derive(Debug, Clone)]
pub struct OWLOntologyDocumentorImpl {
    #[allow(unused)]
    file_source:  Option<FileSourceImplementor>,
    #[allow(unused)]
    file_name:    Option<PathBuf>,
    loader_store: LoaderStore,
    /// The given target documentation model that the
    /// OWLOntologyDocumentor will add its documentation to.
    doc_model:    Arc<Model>,
}

impl DocumentorCreator for OWLOntologyDocumentorImpl {
    fn new(
        file_source: Option<FileSourceImplementor>,
        file_name: Option<&Path>,
        loader_store: LoaderStore,
        doc_model: Arc<Model>,
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
impl Documentor for OWLOntologyDocumentorImpl {
    fn file_types(&self) -> FileTypeSliceStatic {
        *OWL_ONTOLOGY_DOCUMENTOR_FILE_TYPES
    }

    async fn generate(&self) -> anyhow::Result<()> {
        info!("Starting OWL ontology documentation generation");

        // Create a book for the ontology
        let title = self
            .file_name
            .as_ref()
            .and_then(|f| f.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("OWL Ontology")
            .to_string();
        info!("Creating book with title: {}", title);

        let _book = Book::builder_in_model::<Book>(&self.doc_model)?
            .title(Some(title))
            .author(Some("GraphArch".to_string()))
            .build()?;

        info!("Book created successfully");

        // Find all OWL classes in the loader store
        info!("Finding OWL classes in loader store");
        let classes = self.loader_store.find_owl_classes()?;
        info!("Found {} OWL classes", classes.len());

        // Create a section for OWL classes
        info!("Creating section for OWL classes");
        Section::builder_in_model::<Section>(&self.doc_model)?
            .title(Some("OWL Classes".to_string()))
            .description(Some(
                "This section contains documentation for all OWL classes in \
                 the ontology."
                    .to_string(),
            ))
            .build()?;
        info!("Section created successfully");

        // Create chapters for each OWL class
        info!("Creating chapters for OWL classes");
        for class in classes {
            Chapter::builder_in_model::<Chapter>(&self.doc_model)?
                .title(Some(
                    class.label.unwrap_or_else(|| class.iri.clone()),
                ))
                .content(Some(
                    class
                        .comment
                        .as_deref()
                        .unwrap_or("No description available."),
                ))
                .build()?;
        }
        info!("Chapters created successfully");

        Ok(())
    }

    fn file_name(&self) -> Option<&Path> { self.file_name.as_deref() }
}

impl OWLOntologyDocumentorImpl {
    /// Iterate over all OWL classes in the model and call the
    /// given closure or function with an OWLOntologyDocumentor for
    /// each class.
    pub async fn for_each_owl_class<F, Fut>(
        &self,
        mut f: F,
    ) -> anyhow::Result<()>
    where
        // "f" is a closure that can be called multiple times (FnMut),
        // takes an OWLOntologyDocumentor, and returns a future.
        F: FnMut(OWLOntologyDocumentorImpl) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let classes = self.loader_store.find_owl_classes()?;
        let mut futures = Vec::new();

        for _class in classes {
            let documentor = OWLOntologyDocumentorImpl::new(
                self.file_source.clone(),
                self.file_name.as_deref(),
                self.loader_store.clone(),
                self.doc_model.clone(),
            );

            futures.push(f(documentor));
        }

        join_all(futures).await;
        Ok(())
    }
}
