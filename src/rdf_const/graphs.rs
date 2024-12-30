use {
    lazy_static::lazy_static,
    oxrdf::{GraphName, NamedNode},
};

// Define the named graph URI
pub const RDF_GRAPH_FILE_REGISTRY: &str =
    "urn:grapharch:file-registry";

lazy_static! {
    pub static ref OXI_GRAPH_FILE_REGISTRY: GraphName =
        NamedNode::new_unchecked(RDF_GRAPH_FILE_REGISTRY).into();
}
