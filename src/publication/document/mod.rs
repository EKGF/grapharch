//! Specialized RDF-to-fragment documentors sharing a bounded streaming engine.

mod annotated_term;
pub(crate) mod description;
mod resource;
mod use_cases;
use {
    super::{dataset::Dataset, writer::GraphWriter},
    anyhow::Result,
    async_trait::async_trait,
    oxrdf::NamedNode,
    resource::ResourceDocumentor,
};
pub use {
    annotated_term::AnnotatedTermDocumentor,
    use_cases::UseCaseDocumentor,
};

/// A generator of semantic documentation fragments from collected RDF only.
#[async_trait]
pub trait Documentor {
    /// Generate source-linked fragments without deciding publication placement.
    async fn document(&self, writer: &GraphWriter) -> Result<()>;
}

macro_rules! documentor {
    ($name:ident, $class:literal, $label:literal) => {
        #[doc = concat!("Specialized fragment generator for ", $label, ".")]
        pub struct $name(ResourceDocumentor);
        impl $name {
            /// Bind this documentor to a project and pinned collected snapshot.
            pub fn new(
                dataset: Dataset,
                snapshot: NamedNode,
                project: &str,
            ) -> Result<Self> {
                Ok(Self(ResourceDocumentor::new(
                    dataset, snapshot, project, $class, $label,
                )?))
            }

            /// Stable identity of this subject's description fragment.
            pub fn fragment_id(&self, subject: &NamedNode) -> NamedNode {
                self.0.fragment_id(subject)
            }
        }
        #[async_trait]
        impl Documentor for $name {
            async fn document(&self, writer: &GraphWriter) -> Result<()> {
                self.0.document(writer).await
            }
        }
    };
}

documentor!(
    OwlClassDocumentor,
    "http://www.w3.org/2002/07/owl#Class",
    "OWL class"
);
documentor!(
    OwlOntologyDocumentor,
    "http://www.w3.org/2002/07/owl#Ontology",
    "OWL ontology"
);
documentor!(
    OwlObjectPropertyDocumentor,
    "http://www.w3.org/2002/07/owl#ObjectProperty",
    "OWL object property"
);
documentor!(
    OwlDatatypePropertyDocumentor,
    "http://www.w3.org/2002/07/owl#DatatypeProperty",
    "OWL datatype property"
);
documentor!(
    ShaclNodeShapeDocumentor,
    "http://www.w3.org/ns/shacl#NodeShape",
    "SHACL node shape"
);
documentor!(
    ShaclPropertyShapeDocumentor,
    "http://www.w3.org/ns/shacl#PropertyShape",
    "SHACL property shape"
);
