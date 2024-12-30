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
    oxrdf::{Subject, Term},
    std::{path::PathBuf, sync::Arc},
};

// Define the named graph URI
#[allow(unused)]
const FILE_REGISTRY_GRAPH: &str = "urn:grapharch:file-registry";

#[derive(Clone)]
pub struct LoaderStore {
    pub store: Arc<Store>,
}

impl LoaderStore {
    /// Creates a new [`LoaderStore`] with the given store.
    pub fn new(store: Arc<Store>) -> Self { Self { store } }

    /// Creates a new in-memory [`LoaderStore`].
    pub fn new_in_memory() -> anyhow::Result<Self> {
        Ok(Self::new(Arc::new(Store::new()?)))
    }

    pub fn store(&self) -> Arc<Store> { self.store.clone() }

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
        let write =
            FileRegistryWriter::new(self, &graph_name, &file_name);
        write.register().await
    }

    pub async fn insert_quad(
        &self,
        quad: &Quad,
    ) -> anyhow::Result<()> {
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
        self.insert_quad(&Quad::new(
            subject, predicate, object, graph_name,
        ))
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
}

impl std::fmt::Debug for LoaderStore {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "LoaderStore")
    }
}
