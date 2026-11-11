//! Showcase runner definitions identified by RDF resources.
use {
    anyhow::{Result, bail},
    oxrdf::NamedNode,
};

/// The source preparation pipeline selected by a showcase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShowcaseRunner {
    Authored,
    Dprod,
    Mkdocs,
    OntologyFamily,
    Skos,
    UseCases,
    MixedSources,
    Specification,
    W3cSpecification,
}

impl ShowcaseRunner {
    pub(super) fn from_node(node: &NamedNode) -> Result<Self> {
        match node.as_str() {
            "https://ekgf.org/ontology/grapharch#AuthoredShowcaseRunner" => {
                Ok(Self::Authored)
            },
            "https://ekgf.org/ontology/grapharch#DprodShowcaseRunner" => {
                Ok(Self::Dprod)
            },
            "https://ekgf.org/ontology/grapharch#MkdocsShowcaseRunner" => {
                Ok(Self::Mkdocs)
            },
            "https://ekgf.org/ontology/grapharch#OntologyFamilyShowcaseRunner" => {
                Ok(Self::OntologyFamily)
            },
            "https://ekgf.org/ontology/grapharch#SkosShowcaseRunner" => {
                Ok(Self::Skos)
            },
            "https://ekgf.org/ontology/grapharch#UseCasesShowcaseRunner" => {
                Ok(Self::UseCases)
            },
            "https://ekgf.org/ontology/grapharch#MixedSourcesShowcaseRunner" => {
                Ok(Self::MixedSources)
            },
            "https://ekgf.org/ontology/grapharch#SpecificationShowcaseRunner" => {
                Ok(Self::Specification)
            },
            "https://ekgf.org/ontology/grapharch#W3cSpecificationShowcaseRunner" => {
                Ok(Self::W3cSpecification)
            },
            _ => bail!("unsupported showcaseRunner IRI {node}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::ShowcaseRunner,
        oxrdf::{NamedNode, NamedOrBlankNode, Term},
        oxrdfio::{RdfFormat, RdfParser},
    };

    const RUNNER: &str = "https://ekgf.org/ontology/grapharch#ShowcaseRunner";
    const TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

    #[test]
    fn every_catalogued_showcase_runner_is_supported() {
        let mut count = 0;
        for quad in RdfParser::from_format(RdfFormat::Turtle).for_slice(
            include_str!("../../docs/other/ontology/showcase-runners.ttl")
                .as_bytes(),
        ) {
            let quad = quad.expect("catalogue parses");
            if quad.predicate.as_str() == TYPE &&
                quad.object ==
                    Term::NamedNode(NamedNode::new_unchecked(RUNNER))
            {
                let NamedOrBlankNode::NamedNode(node) = quad.subject else {
                    panic!("showcase runners need IRIs")
                };
                ShowcaseRunner::from_node(&node).expect("supported runner");
                count += 1;
            }
        }
        assert_eq!(count, 9);
    }

    #[test]
    fn unknown_showcase_runners_are_rejected() {
        for iri in [
            "https://ekgf.org/ontology/grapharch#ShowcaseRunner",
            "urn:grapharch:showcase:authored",
            "urn:custom:DprodShowcaseRunner",
        ] {
            assert!(
                ShowcaseRunner::from_node(&NamedNode::new_unchecked(iri))
                    .is_err(),
                "accepted {iri}"
            );
        }
    }
}
