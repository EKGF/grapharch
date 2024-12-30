use {
    super::namespaces::NS_XSD,
    const_format::concatcp,
    lazy_static::lazy_static,
    oxrdf::NamedNode,
};

static RDF_XSD_DATETIME: &str = concatcp!(NS_XSD, "dateTime");

static RDF_XSD_INTEGER: &str = concatcp!(NS_XSD, "integer");

static RDF_XSD_STRING: &str = concatcp!(NS_XSD, "string");

lazy_static! {
    pub static ref OXI_RDF_XSD_DATETIME: NamedNode =
        NamedNode::new(RDF_XSD_DATETIME).unwrap();
    pub static ref OXI_RDF_XSD_INTEGER: NamedNode =
        NamedNode::new(RDF_XSD_INTEGER).unwrap();
    pub static ref OXI_RDF_XSD_STRING: NamedNode =
        NamedNode::new(RDF_XSD_STRING).unwrap();
}
