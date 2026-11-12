//! Canonical documentor definitions, distinct from project-bound uses.
use {
    anyhow::{Result, bail},
    oxrdf::NamedNode,
};

/// A documentor definition selectable by a configured `ga:DocumentorUse`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentorKind {
    MaturityModel,
    MaturityOverview,
    PublicationInventory,
    VocabularyReference,
}

impl DocumentorKind {
    pub(crate) const PUBLICATION_INVENTORY: &'static str =
        "https://ekgf.org/ontology/grapharch#PublicationInventoryDocumentor";

    pub(crate) fn from_node(node: &NamedNode) -> Result<Self> {
        match node.as_str() {
            "https://ekgf.org/ontology/grapharch#MaturityModelDocumentor" => {
                Ok(Self::MaturityModel)
            },
            "https://ekgf.org/ontology/grapharch#MaturityOverviewDocumentor" => {
                Ok(Self::MaturityOverview)
            },
            Self::PUBLICATION_INVENTORY => Ok(Self::PublicationInventory),
            "https://ekgf.org/ontology/grapharch#VocabularyReferenceDocumentor" => {
                Ok(Self::VocabularyReference)
            },
            _ => bail!("unsupported documentorType IRI {node}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::DocumentorKind,
        oxrdf::{NamedNode, NamedOrBlankNode, Term},
        oxrdfio::{RdfFormat, RdfParser},
    };

    const DOCUMENTOR: &str = "https://ekgf.org/ontology/grapharch#Documentor";
    const TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

    #[test]
    fn every_catalogued_documentor_definition_is_supported() {
        let mut count = 0;
        for quad in RdfParser::from_format(RdfFormat::Turtle).for_slice(
            include_str!("../../../docs/other/ontology/documentors.ttl")
                .as_bytes(),
        ) {
            let quad = quad.expect("catalogue parses");
            if quad.predicate.as_str() == TYPE &&
                quad.object ==
                    Term::NamedNode(NamedNode::new_unchecked(DOCUMENTOR))
            {
                let NamedOrBlankNode::NamedNode(node) = quad.subject else {
                    panic!("documentor definitions need IRIs")
                };
                DocumentorKind::from_node(&node).expect("supported definition");
                count += 1;
            }
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn unknown_documentor_definitions_are_rejected() {
        for iri in [
            "https://ekgf.org/ontology/grapharch#Documentor",
            "urn:grapharch:documentor:publication-inventory",
            "urn:custom:MaturityModelDocumentor",
        ] {
            assert!(
                DocumentorKind::from_node(&NamedNode::new_unchecked(iri))
                    .is_err(),
                "accepted {iri}"
            );
        }
    }
}
