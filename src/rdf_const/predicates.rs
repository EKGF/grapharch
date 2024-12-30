use {
    super::namespaces::{NS_FILE_REGISTRY, NS_RDF, NS_RDFS},
    const_format::concatcp,
    lazy_static::lazy_static,
    oxrdf::NamedNode,
};

static PREDICATE_RDF_TYPE: &str = concatcp!(NS_RDF, "type");

static PREDICATE_RDFS_LABEL: &str = concatcp!(NS_RDFS, "label");

static PREDICATE_FILE_REGISTRY_IS_CONTENT_FOR_FILE: &str =
    concatcp!(NS_FILE_REGISTRY, "isContentForFile");

static PREDICATE_FILE_REGISTRY_FILE_SIZE: &str =
    concatcp!(NS_FILE_REGISTRY, "fileSize");

static PREDICATE_FILE_REGISTRY_LAST_MODIFIED: &str =
    concatcp!(NS_FILE_REGISTRY, "lastModified");

static PREDICATE_FILE_REGISTRY_CREATED_AT: &str =
    concatcp!(NS_FILE_REGISTRY, "createdAt");

lazy_static! {
    pub static ref OXI_RDF_TYPE: NamedNode =
        NamedNode::new_unchecked(PREDICATE_RDF_TYPE);
    pub static ref OXI_RDFS_LABEL: NamedNode =
        NamedNode::new_unchecked(PREDICATE_RDFS_LABEL);
    pub static ref OXI_FILE_REGISTRY_IS_CONTENT_FOR_FILE: NamedNode =
        NamedNode::new_unchecked(PREDICATE_FILE_REGISTRY_IS_CONTENT_FOR_FILE);
    pub static ref OXI_FILE_REGISTRY_FILE_SIZE: NamedNode =
        NamedNode::new_unchecked(PREDICATE_FILE_REGISTRY_FILE_SIZE);
    pub static ref OXI_FILE_REGISTRY_LAST_MODIFIED: NamedNode =
        NamedNode::new_unchecked(PREDICATE_FILE_REGISTRY_LAST_MODIFIED);
    pub static ref OXI_FILE_REGISTRY_CREATED_AT: NamedNode =
        NamedNode::new_unchecked(PREDICATE_FILE_REGISTRY_CREATED_AT);
}
