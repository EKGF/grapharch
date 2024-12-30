use {
    super::namespaces::NS_FILE_REGISTRY,
    const_format::concatcp,
    lazy_static::lazy_static,
    oxrdf::NamedNode,
};
pub static CLASS_FILE_REGISTRY_FILE: &str =
    concatcp!(NS_FILE_REGISTRY, "File");

pub static CLASS_FILE_REGISTRY_FILE_DISTRIBUTION: &str =
    concatcp!(NS_FILE_REGISTRY, "FileDistribution");

lazy_static! {
    pub static ref OXI_CLASS_FILE_REGISTRY_FILE: NamedNode =
        NamedNode::new_unchecked(CLASS_FILE_REGISTRY_FILE);
    pub static ref OXI_CLASS_FILE_REGISTRY_FILE_DISTRIBUTION: NamedNode =
        NamedNode::new_unchecked(
            CLASS_FILE_REGISTRY_FILE_DISTRIBUTION
        );
}
