use {
    super::file_registry::FileRegistryWriter,
    crate::rdf_const::{
        graphs::OXI_GRAPH_FILE_REGISTRY,
        predicates::OXI_RDF_TYPE,
    },
    oxigraph::{
        model::{GraphName, NamedNode, Quad},
        store::Store,
    },
    oxrdf::{Subject, Term, TermRef},
    std::{path::PathBuf, sync::Arc},
};

/// Represents an OWL class from the source data
#[derive(Debug, Clone)]
pub struct OWLClass {
    /// The IRI of the OWL class
    pub iri:     String,
    /// The human-readable label of the class
    pub label:   Option<String>,
    /// A description or comment about the class
    pub comment: Option<String>,
}

// Define the named graph URI
#[allow(unused)]
const FILE_REGISTRY_GRAPH: &str = "urn:GraphArch:file-registry";

#[derive(Clone)]
pub struct LoaderStore {
    store: Arc<Store>,
}

/// A store for loading and storing data.
///
/// The `LoaderStore` is a wrapper around an `Arc<Store>`, which
/// is a reference-counted pointer to a `Store`. The `Store` is
/// an in-memory graph database that is used to store the data
/// that is loaded from the file system.
///
/// The `LoaderStore` is used to load and store data from the file
/// system or any other supported `FileSource` implementation.
/// It loads all the data from the given `FileSource` for further
/// processing by the `Documentor`s who have been registered to
/// cherry-pick the "documentable things" from the loaded data
/// and transform those into a documentation model in another
/// store called `DocumentationModel` (or actually that's a wrapper
/// around a `Store`).
impl LoaderStore {
    /// Creates a new [`LoaderStore`] with the given store.
    pub fn new(store: Arc<Store>) -> Self { Self { store } }

    /// Creates a new in-memory [`LoaderStore`].
    pub fn new_in_memory() -> anyhow::Result<Self> {
        Ok(Self::new(Arc::new(Store::new()?)))
    }

    pub fn store(&self) -> Arc<Store> { self.store.clone() }

    /// Debug function to print all triples in the store
    pub fn debug_print_all_triples(&self) -> anyhow::Result<()> {
        let query = r#"
            SELECT ?s ?p ?o ?g
            WHERE {
                GRAPH ?g {
                    ?s ?p ?o .
                }
            }
            LIMIT 100
        "#;

        let results = self.store.query(query)?;

        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            println!("First 100 triples in the store:");
            for solution in solutions {
                let solution = solution?;
                if let (Some(s), Some(p), Some(o), Some(g)) = (
                    solution.get("s"),
                    solution.get("p"),
                    solution.get("o"),
                    solution.get("g"),
                ) {
                    println!("Graph: {}", g);
                    println!("  {} {} {}", s, p, o);
                }
            }
        }

        Ok(())
    }

    /// Returns the graph name for the file registry.
    fn graph_name_file_registry(&self) -> GraphName {
        OXI_GRAPH_FILE_REGISTRY.clone()
    }

    /// Registers a file in the store and returns a tuple with the
    /// file content and the chosen identifier for the file.
    ///
    /// The identifier is chosen based on a SHA-256 hash of the file
    /// content, ensuring a unique and reliable identifier for
    /// each file. Each changed file results in a new node in the
    /// store. For each unique file content, the full file path
    /// (relative to the given root directory) is also registered
    /// as a node and linked to the file content node.
    ///
    /// This allows retrieval of the file content by its identifier
    /// and the full file path by its identifier.
    ///
    /// Additionally, all 'stats' of the given file are registered as
    /// triples (of the file content node) in the store, enabling
    /// retrieval by their file content node identifier.
    pub async fn register_file(
        &self,
        file_name: PathBuf,
    ) -> anyhow::Result<(String, NamedNode)> {
        let graph_name = self.graph_name_file_registry();
        let write = FileRegistryWriter::new(self, &graph_name, &file_name);
        write.register().await
    }

    pub async fn insert_quad(&self, quad: &Quad) -> anyhow::Result<()> {
        self.store.insert(quad)?;
        Ok(())
    }

    pub async fn insert_quad2(
        &self,
        subject: impl Into<Subject>,
        predicate: impl Into<NamedNode>,
        object: impl Into<Term>,
        graph_name: impl Into<GraphName>,
    ) -> anyhow::Result<()> {
        self.insert_quad(&Quad::new(subject, predicate, object, graph_name))
            .await
    }

    pub async fn insert_node_of_type(
        &self,
        node: NamedNode,
        node_type: NamedNode,
        graph_name: GraphName,
    ) -> anyhow::Result<()> {
        self.store.insert(&Quad::new(
            node,
            OXI_RDF_TYPE.clone(),
            node_type,
            graph_name,
        ))?;

        Ok(())
    }

    /// Find all OWL classes in the store and return them as OWLClass structs.
    pub fn find_owl_classes(&self) -> anyhow::Result<Vec<OWLClass>> {
        let query = r#"
            PREFIX owl: <http://www.w3.org/2002/07/owl#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT DISTINCT ?class ?label ?comment
            WHERE {
                GRAPH ?g {
                    {
                        ?class a owl:Class .
                    } UNION {
                        ?class a rdfs:Class .
                    } UNION {
                        # Some ontologies use rdf:type directly
                        ?class rdf:type owl:Class .
                    } UNION {
                        ?class rdf:type rdfs:Class .
                    }
                    OPTIONAL { ?class rdfs:label ?label }
                    OPTIONAL { ?class rdfs:comment ?comment }
                }
            }
            ORDER BY ?class
        "#;

        let results = self.store.query(query)?;
        let mut classes = Vec::new();

        match results {
            oxigraph::sparql::QueryResults::Solutions(solutions) => {
                for solution in solutions {
                    let solution = solution?;

                    let class = solution
                        .get("class")
                        .and_then(|t| {
                            match t.as_ref() {
                                TermRef::NamedNode(n) => Some(n),
                                _ => None,
                            }
                        })
                        .ok_or_else(|| {
                            anyhow::anyhow!("Expected named node for class")
                        })?;

                    let label = solution.get("label").and_then(|t| {
                        match t.as_ref() {
                            TermRef::Literal(l) => Some(l.value().to_string()),
                            _ => None,
                        }
                    });

                    let comment = solution.get("comment").and_then(|t| {
                        match t.as_ref() {
                            TermRef::Literal(l) => Some(l.value().to_string()),
                            _ => None,
                        }
                    });

                    classes.push(OWLClass {
                        iri: class.to_string(),
                        label,
                        comment,
                    });
                }
            },
            _ => return Err(anyhow::anyhow!("Unexpected query results type")),
        }

        Ok(classes)
    }

    /// Debug function to check if there are any classes in the ontology
    pub fn debug_check_for_any_classes(&self) -> anyhow::Result<()> {
        // Try different class patterns
        let patterns = [
            "?class a ?type . FILTER(CONTAINS(STR(?type), 'Class'))",
            "?class <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ?type . \
             FILTER(CONTAINS(STR(?type), 'Class'))",
            "?class a ?type . FILTER(CONTAINS(STR(?type), 'class'))",
            "?class <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ?type . \
             FILTER(CONTAINS(STR(?type), 'class'))",
            "?class a ?type .",
            "?class <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ?type .",
        ];

        for pattern in patterns {
            let query = format!(
                r#"
                SELECT DISTINCT ?class ?type
                WHERE {{
                    GRAPH ?g {{
                        {}
                    }}
                }}
                LIMIT 10
            "#,
                pattern
            );

            let results = self.store.query(&query)?;

            if let oxigraph::sparql::QueryResults::Solutions(solutions) =
                results
            {
                let solutions: Vec<_> = solutions.collect::<Result<_, _>>()?;
                if !solutions.is_empty() {
                    println!("Found classes using pattern: {}", pattern);
                    for solution in solutions {
                        if let (Some(class), Some(type_)) =
                            (solution.get("class"), solution.get("type"))
                        {
                            println!("  Class: {}, Type: {}", class, type_);
                        }
                    }
                } else {
                    println!("No classes found using pattern: {}", pattern);
                }
            }
        }

        Ok(())
    }
}

impl std::fmt::Debug for LoaderStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LoaderStore")
    }
}
