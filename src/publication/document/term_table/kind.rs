//! The OWL entity kinds a term table documents, recognized from declared
//! RDF types.
use oxrdf::NamedNode;

const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// The entity kind whose axioms one table row renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TermKind {
    Class,
    Datatype,
    Property,
    Individual,
}

impl TermKind {
    /// The declared types that only state a term's kind; they are not shown
    /// as axioms.
    pub fn is_declaration(kind: &NamedNode) -> bool {
        matches!(
            kind.as_str().strip_prefix(OWL),
            Some(
                "Class" |
                    "ObjectProperty" |
                    "DatatypeProperty" |
                    "AnnotationProperty" |
                    "NamedIndividual"
            )
        ) || matches!(
            kind.as_str().strip_prefix(RDFS),
            Some("Class" | "Datatype")
        ) || kind.as_str() == format!("{RDF}Property")
    }

    /// Property characteristics, shown as property types.
    pub fn is_characteristic(kind: &NamedNode) -> bool {
        matches!(
            kind.as_str().strip_prefix(OWL),
            Some(
                "FunctionalProperty" |
                    "InverseFunctionalProperty" |
                    "TransitiveProperty" |
                    "SymmetricProperty" |
                    "AsymmetricProperty" |
                    "ReflexiveProperty" |
                    "IrreflexiveProperty"
            )
        )
    }

    /// The kind whose axioms a term with these declared types shows. Named
    /// individuals stay individuals even when punned.
    pub fn of(types: &[NamedNode]) -> Option<Self> {
        let has = |namespace: &str, local: &str| {
            types.iter().any(|kind| {
                kind.as_str().strip_prefix(namespace) == Some(local)
            })
        };
        if has(OWL, "NamedIndividual") {
            Some(Self::Individual)
        } else if has(RDFS, "Datatype") {
            Some(Self::Datatype)
        } else if has(OWL, "Class") || has(RDFS, "Class") {
            Some(Self::Class)
        } else if has(OWL, "ObjectProperty") ||
            has(OWL, "DatatypeProperty") ||
            has(OWL, "AnnotationProperty") ||
            has(RDF, "Property") ||
            types.iter().any(Self::is_characteristic)
        {
            Some(Self::Property)
        } else {
            None
        }
    }

    /// The heading of the axiom column for rows of this kind.
    pub fn axiom_column(self) -> &'static str {
        match self {
            Self::Class | Self::Datatype => "Class Expressions",
            Self::Property => "Property Axioms",
            Self::Individual => "Individual Axioms",
        }
    }
}
