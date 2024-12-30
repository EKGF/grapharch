mod file;
mod file_type;
mod paths;
mod rdf_load;
mod tracing;

pub use {
    file::contents_of_local_file,
    file_type::{FileType, FileTypeSlice, FileTypeSliceStatic},
    paths::{derive_root_directory, relative_path},
    rdf_load::rdf_load,
    tracing::setup_tracing,
};
use {oxigraph::model::NamedNode, oxrdf::NamedNodeRef, uuid::Uuid};

pub fn extract_local_name(iri: NamedNodeRef) -> String {
    iri.as_str()
        .rsplit(&['/', '#'][..])
        .next()
        .unwrap_or("")
        .to_string()
}

/// Generates a UUID URN and returns it as a NamedNode.
pub fn generate_uuid_named_node() -> anyhow::Result<NamedNode> {
    let uuid = Uuid::new_v4();
    let urn = format!("urn:uuid:{}", uuid);
    Ok(NamedNode::new(urn)?)
}
