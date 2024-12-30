use {
    crate::{
        documentor::{
            Documentor,
            DocumentorCreator,
            DocumentorImplementor,
            this::DocumentorVariant,
        },
        model::DocumentationModel,
        source::FileSourceImplementor,
        store::LoaderStore,
        util::{FileType, FileTypeSliceStatic},
    },
    async_trait::async_trait,
    futures::future::join_all,
    oxigraph::sparql::QueryResults,
    oxrdf::TermRef,
    std::{
        path::{Path, PathBuf},
        sync::{Arc, LazyLock},
    },
};

static OWL_ONTOLOGY_DOCUMENTOR_FILE_TYPES: LazyLock<
    FileTypeSliceStatic,
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
    doc_model:    DocumentationModel,
}

impl DocumentorCreator for OWLOntologyDocumentorImpl {
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
impl Documentor for OWLOntologyDocumentorImpl {
    fn file_types(&self) -> FileTypeSliceStatic {
        *OWL_ONTOLOGY_DOCUMENTOR_FILE_TYPES
    }

    async fn generate(&self) -> anyhow::Result<()> { todo!() }

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
        let query = r#"
            PREFIX owl: <http://www.w3.org/2002/07/owl#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            SELECT DISTINCT ?graph ?resource ?class WHERE {
                GRAPH ?graph {
                    ?resource a ?class .
                    FILTER(?class IN (owl:Class, rdfs:Class))
                }
            }
        "#;

        // We need to collect the futures that are going to be created
        // for each OWL class that we're going to process, so we can
        // wait for them...
        let mut futures: Vec<Fut> = Vec::new();

        // DocumentationModel is Clone, we can do:
        let _doc_model_arc = Arc::new(self.clone());

        if let QueryResults::Solutions(solutions) =
            self.loader_store.store.query(query)?
        {
            for solution in solutions {
                if let Some(class_iri) = solution?.get("resource") {
                    if let TermRef::NamedNode(_class_iri) =
                        class_iri.as_ref()
                    {
                        let documentor_enum_type =
                            DocumentorImplementor::new(
                                DocumentorVariant::OWLOntology,
                                None,
                                None,
                                self.loader_store.clone(), /* class_iri.into_owned(), */
                                self.doc_model.clone(),
                            );

                        if let DocumentorImplementor::OWLOntologyDocumentor(
                            owl_ontology_documentor,
                        ) = documentor_enum_type
                        {
                            let future = f(owl_ontology_documentor);
                            futures.push(future);
                        } else {
                            tracing::warn!("unknown documentor type");
                        }
                    }
                }
            }
        }

        // Run all futures concurrently and wait for them to complete
        join_all(futures)
            .await
            .into_iter()
            .collect::<anyhow::Result<()>>()?;

        Ok(())
    }
}
