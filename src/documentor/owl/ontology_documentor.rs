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

        // Debug: Print all triples in the store to help diagnose issues
        info!("Debugging: Printing sample triples from the store");
        if let Err(e) = self.loader_store.debug_print_all_triples() {
            info!("Error printing triples: {}", e);
        }

        // Debug: Check for any classes in the ontology
        info!("Debugging: Checking for any classes in the ontology");
        if let Err(e) = self.loader_store.debug_check_for_any_classes() {
            info!("Error checking for classes: {}", e);
        }

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

        // Create a section for OWL classes
        info!("Creating section for OWL classes");
        let section = Section::builder_in_model::<Section>(&self.doc_model)?
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
        let classes = self.loader_store.find_owl_classes()?;

        if classes.is_empty() {
            // If no classes were found, create a default chapter with
            // information about the ontology
            info!(
                "No OWL classes found, creating a default chapter with \
                 ontology information"
            );

            // Try to get ontology information
            let ontology_info = self.get_ontology_info()?;

            let content = if !ontology_info.is_empty() {
                format!("# Ontology Information\n\n{}", ontology_info)
            } else {
                "No classes were found in this ontology. The file may not \
                 contain OWL class definitions, or they may be defined using a \
                 different vocabulary."
                    .to_string()
            };

            Chapter::builder_in_model::<Chapter>(&self.doc_model)?
                .title(Some("Ontology Overview".to_string()))
                .content(Some(content.as_str()))
                .build()?;
        } else {
            // Create chapters for each OWL class
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
        }
        info!("Chapters created successfully");

        Ok(())
    }

    fn file_name(&self) -> Option<&Path> { self.file_name.as_deref() }
}

impl OWLOntologyDocumentorImpl {
    /// Get information about the ontology itself
    fn get_ontology_info(&self) -> anyhow::Result<String> {
        let query = r#"
            PREFIX owl: <http://www.w3.org/2002/07/owl#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX dc: <http://purl.org/dc/elements/1.1/>
            PREFIX dcterms: <http://purl.org/dc/terms/>

            SELECT ?ontology ?title ?label ?comment ?description ?creator ?date ?version
            WHERE {
                GRAPH ?g {
                    ?ontology a owl:Ontology .
                    OPTIONAL { ?ontology dc:title ?title }
                    OPTIONAL { ?ontology dcterms:title ?title }
                    OPTIONAL { ?ontology rdfs:label ?label }
                    OPTIONAL { ?ontology rdfs:comment ?comment }
                    OPTIONAL { ?ontology dc:description ?description }
                    OPTIONAL { ?ontology dcterms:description ?description }
                    OPTIONAL { ?ontology dc:creator ?creator }
                    OPTIONAL { ?ontology dcterms:creator ?creator }
                    OPTIONAL { ?ontology dc:date ?date }
                    OPTIONAL { ?ontology dcterms:date ?date }
                    OPTIONAL { ?ontology owl:versionInfo ?version }
                }
            }
            LIMIT 1
        "#;

        let results = self.loader_store.store().query(query)?;
        let mut info = String::new();

        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            let solutions: Vec<_> = solutions.collect::<Result<_, _>>()?;
            if !solutions.is_empty() {
                let solution = &solutions[0];

                if let Some(ontology) = solution.get("ontology") {
                    info.push_str(&format!(
                        "## Ontology IRI\n\n{}\n\n",
                        ontology
                    ));
                }

                // Try to get title from various properties
                if let Some(title) =
                    solution.get("title").or(solution.get("label"))
                {
                    info.push_str(&format!("## Title\n\n{}\n\n", title));
                }

                // Try to get description from various properties
                if let Some(desc) =
                    solution.get("comment").or(solution.get("description"))
                {
                    info.push_str(&format!("## Description\n\n{}\n\n", desc));
                }

                // Add creator if available
                if let Some(creator) = solution.get("creator") {
                    info.push_str(&format!("## Creator\n\n{}\n\n", creator));
                }

                // Add date if available
                if let Some(date) = solution.get("date") {
                    info.push_str(&format!("## Date\n\n{}\n\n", date));
                }

                // Add version if available
                if let Some(version) = solution.get("version") {
                    info.push_str(&format!("## Version\n\n{}\n\n", version));
                }
            }
        }

        if info.is_empty() {
            // If we couldn't find ontology info, try to get any triples
            info.push_str(
                "No detailed ontology information found. Here are some sample \
                 triples from the file:\n\n",
            );

            let query = r#"
                SELECT ?s ?p ?o
                WHERE {
                    GRAPH ?g {
                        ?s ?p ?o .
                    }
                }
                LIMIT 10
            "#;

            let results = self.loader_store.store().query(query)?;

            if let oxigraph::sparql::QueryResults::Solutions(solutions) =
                results
            {
                let solutions: Vec<_> = solutions.collect::<Result<_, _>>()?;
                if !solutions.is_empty() {
                    for solution in solutions {
                        if let (Some(s), Some(p), Some(o)) = (
                            solution.get("s"),
                            solution.get("p"),
                            solution.get("o"),
                        ) {
                            info.push_str(&format!(
                                "- Subject: {}\n  Predicate: {}\n  Object: \
                                 {}\n\n",
                                s, p, o
                            ));
                        }
                    }
                }
            }
        }

        Ok(info)
    }

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
