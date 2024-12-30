use {
    crate::{
        rdf_const::{
            classes::OXI_CLASS_FILE_REGISTRY_FILE,
            data_types::{OXI_RDF_XSD_DATETIME, OXI_RDF_XSD_INTEGER},
            predicates::{
                OXI_FILE_REGISTRY_CREATED_AT,
                OXI_FILE_REGISTRY_FILE_SIZE,
                OXI_FILE_REGISTRY_IS_CONTENT_FOR_FILE,
                OXI_FILE_REGISTRY_LAST_MODIFIED,
                OXI_RDFS_LABEL,
            },
        },
        store::LoaderStore,
    },
    oxrdf::{GraphName, Literal, NamedNode, Quad, Subject, Term},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
    tokio::{fs, io::AsyncReadExt, time::Instant},
};

/// The `FileReqistryWriter` registers a file in the given
/// `LoaderStore` and returns the chosen identifier for the file.
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
pub(crate) struct FileRegistryWriter<'a> {
    store:      &'a LoaderStore,
    graph_name: &'a GraphName,
    file_name:  &'a PathBuf,
    start_time: Instant,
}

impl<'a> FileRegistryWriter<'a> {
    pub fn new(
        store: &'a LoaderStore,
        graph_name: &'a GraphName,
        file_name: &'a PathBuf,
    ) -> Self {
        Self {
            store,
            graph_name,
            file_name,
            start_time: Instant::now(),
        }
    }

    async fn insert_node_of_type(
        &self,
        node: NamedNode,
        node_type: NamedNode,
    ) -> anyhow::Result<()> {
        self.store
            .insert_node_of_type(node, node_type, self.graph_name.clone())
            .await
    }

    pub async fn insert_quad2(
        &self,
        subject: impl Into<Subject>,
        predicate: impl Into<NamedNode>,
        object: impl Into<Term>,
    ) -> anyhow::Result<()> {
        self.store
            .insert_quad(&Quad::new(
                subject,
                predicate,
                object,
                self.graph_name.clone(),
            ))
            .await
    }

    pub async fn register(&self) -> anyhow::Result<(String, NamedNode)> {
        // Open the file asynchronously
        let mut file = fs::File::open(&self.file_name).await?;

        // Read the file content asynchronously into a String
        let mut file_content_str = String::new();
        file.read_to_string(&mut file_content_str).await?;

        // Compute the SHA-256 hash of the file content string
        let mut hasher = Sha256::new();
        hasher.update(file_content_str.as_bytes());
        let hash_result = hasher.finalize();
        let file_hash = format!("{:x}", hash_result);

        // Create a NamedNode for the file content using the hash
        let file_content_node =
            NamedNode::new(format!("urn:sha256:{}", file_hash))?;

        // Create a NamedNode for the file path
        let file_name_node = NamedNode::new(format!(
            "urn:file:{}",
            self.file_name.to_string_lossy()
        ))?;
        self.insert_node_of_type(
            file_name_node.clone(),
            OXI_CLASS_FILE_REGISTRY_FILE.clone(),
        )
        .await?;
        self.insert_quad2(
            file_content_node.clone(),
            OXI_FILE_REGISTRY_IS_CONTENT_FOR_FILE.clone(),
            file_name_node.clone(),
        )
        .await?;

        self.insert_quad2(
            file_name_node.clone(),
            OXI_RDFS_LABEL.clone(),
            Literal::new_simple_literal(&*self.file_name.to_string_lossy()),
        )
        .await?;

        // Collect file stats
        let metadata = file.metadata().await?;

        // Register file stats as triples
        let file_stats = vec![
            (
                OXI_FILE_REGISTRY_FILE_SIZE.clone(),
                Literal::new_typed_literal(
                    metadata.len().to_string(),
                    OXI_RDF_XSD_INTEGER.clone(),
                ),
            ),
            (
                OXI_FILE_REGISTRY_LAST_MODIFIED.clone(),
                Literal::new_typed_literal(
                    format!("{:?}", metadata.modified()?),
                    OXI_RDF_XSD_DATETIME.clone(),
                ),
            ),
            (
                OXI_FILE_REGISTRY_CREATED_AT.clone(),
                Literal::new_typed_literal(
                    format!("{:?}", metadata.created()?),
                    OXI_RDF_XSD_DATETIME.clone(),
                ),
            ),
        ];

        for (predicate, object) in file_stats {
            self.insert_quad2(
                file_content_node.clone(),
                predicate.clone(),
                object,
            )
            .await?;
        }

        // Log the time taken for file reading and stats collection
        let elapsed_time = self.start_time.elapsed();
        println!(
            "Time taken to read file and collect stats: {:.2?}",
            elapsed_time
        );

        Ok((file_content_str, file_content_node))
    }
}
