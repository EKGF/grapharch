//! Which ontology a diagram resource belongs to, compared with the focus term.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeSet,
};

const IS_DEFINED_BY: &str = "http://www.w3.org/2000/01/rdf-schema#isDefinedBy";

/// The authored `rdfs:isDefinedBy` ontologies of one resource and its IRI
/// namespace. Two resources share an ontology when both declare defining
/// ontologies and these intersect, or, when either declares none, when their
/// namespaces are equal.
pub(super) struct OntologyMembership {
    defined_by: BTreeSet<String>,
    namespace:  String,
}

impl OntologyMembership {
    pub async fn read(view: &RevisionView, id: &NamedNode) -> Result<Self> {
        // bounded: at most eight authored defining ontologies per resource.
        let mut defined_by = BTreeSet::new();
        let mut rows = view
            .objects_by(id, &NamedNode::new_unchecked(IS_DEFINED_BY))
            .await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(ontology) = &row["value"] else {
                anyhow::bail!("rdfs:isDefinedBy of {id} must be an IRI");
            };
            defined_by.insert(ontology.as_str().to_owned());
            ensure!(
                defined_by.len() <= 8,
                "{id} exceeds the eight defining ontology budget"
            );
        }
        Ok(Self {
            defined_by,
            namespace: Self::namespace(id.as_str()).into(),
        })
    }

    fn namespace(iri: &str) -> &str {
        iri.rfind(['#', '/']).map_or(iri, |end| &iri[..=end])
    }

    pub fn shares_ontology_with(&self, other: &Self) -> bool {
        if self.defined_by.is_empty() || other.defined_by.is_empty() {
            self.namespace == other.namespace
        } else {
            !self.defined_by.is_disjoint(&other.defined_by)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership(namespace: &str, defined_by: &[&str]) -> OntologyMembership {
        OntologyMembership {
            defined_by: defined_by.iter().map(|s| (*s).into()).collect(),
            namespace:  namespace.into(),
        }
    }

    #[test]
    fn defining_ontologies_take_precedence_over_namespaces() {
        let a = membership("urn:x/", &["urn:a"]);
        assert!(!a.shares_ontology_with(&membership("urn:x/", &["urn:b"])));
        assert!(a.shares_ontology_with(&membership("urn:y/", &["urn:a"])));
        assert!(a.shares_ontology_with(&membership("urn:x/", &[])));
        assert!(!a.shares_ontology_with(&membership("urn:y/", &[])));
        assert_eq!(
            OntologyMembership::namespace("https://e.org/o#T"),
            "https://e.org/o#"
        );
    }
}
