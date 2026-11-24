//! The OWL constructor vocabulary of anonymous class expressions.
use super::model::Bound;

const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// One OWL predicate of an anonymous expression node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Constructor {
    OnProperty,
    OnClass,
    OnDataRange,
    SomeValuesFrom,
    AllValuesFrom,
    HasValue,
    HasSelf,
    MinCardinality,
    Cardinality,
    MaxCardinality,
    MinQualifiedCardinality,
    QualifiedCardinality,
    MaxQualifiedCardinality,
    UnionOf,
    IntersectionOf,
    ComplementOf,
    DatatypeComplementOf,
    OneOf,
    InverseOf,
}
impl Constructor {
    const ALL: [Self; 19] = [
        Self::OnProperty,
        Self::OnClass,
        Self::OnDataRange,
        Self::SomeValuesFrom,
        Self::AllValuesFrom,
        Self::HasValue,
        Self::HasSelf,
        Self::MinCardinality,
        Self::Cardinality,
        Self::MaxCardinality,
        Self::MinQualifiedCardinality,
        Self::QualifiedCardinality,
        Self::MaxQualifiedCardinality,
        Self::UnionOf,
        Self::IntersectionOf,
        Self::ComplementOf,
        Self::DatatypeComplementOf,
        Self::OneOf,
        Self::InverseOf,
    ];

    /// The constructor a predicate IRI names, if any.
    pub fn parse(iri: &str) -> Option<Self> {
        let local = iri.strip_prefix(OWL)?;
        Self::ALL.into_iter().find(|term| term.local() == local)
    }

    fn local(self) -> &'static str {
        match self {
            Self::OnProperty => "onProperty",
            Self::OnClass => "onClass",
            Self::OnDataRange => "onDataRange",
            Self::SomeValuesFrom => "someValuesFrom",
            Self::AllValuesFrom => "allValuesFrom",
            Self::HasValue => "hasValue",
            Self::HasSelf => "hasSelf",
            Self::MinCardinality => "minCardinality",
            Self::Cardinality => "cardinality",
            Self::MaxCardinality => "maxCardinality",
            Self::MinQualifiedCardinality => "minQualifiedCardinality",
            Self::QualifiedCardinality => "qualifiedCardinality",
            Self::MaxQualifiedCardinality => "maxQualifiedCardinality",
            Self::UnionOf => "unionOf",
            Self::IntersectionOf => "intersectionOf",
            Self::ComplementOf => "complementOf",
            Self::DatatypeComplementOf => "datatypeComplementOf",
            Self::OneOf => "oneOf",
            Self::InverseOf => "inverseOf",
        }
    }

    /// Whether the predicate states what a restriction requires.
    pub fn is_constraint(self) -> bool {
        matches!(
            self,
            Self::SomeValuesFrom |
                Self::AllValuesFrom |
                Self::HasValue |
                Self::HasSelf
        ) || self.bound().is_some()
    }

    /// The bound and whether it is qualified, for a cardinality predicate.
    pub fn bound(self) -> Option<(Bound, bool)> {
        Some(match self {
            Self::MinCardinality => (Bound::Minimum, false),
            Self::Cardinality => (Bound::Exact, false),
            Self::MaxCardinality => (Bound::Maximum, false),
            Self::MinQualifiedCardinality => (Bound::Minimum, true),
            Self::QualifiedCardinality => (Bound::Exact, true),
            Self::MaxQualifiedCardinality => (Bound::Maximum, true),
            _ => return None,
        })
    }
}
/// Facts of an anonymous expression node that carry no class semantics.
pub(super) struct Annotation;
impl Annotation {
    const PREDICATES: [&'static str; 5] = [
        "http://www.w3.org/2000/01/rdf-schema#label",
        "http://www.w3.org/2000/01/rdf-schema#comment",
        "http://www.w3.org/2004/02/skos/core#prefLabel",
        "http://www.w3.org/2004/02/skos/core#definition",
        "http://purl.org/dc/terms/description",
    ];
    const TYPES: [&'static str; 4] = [
        "http://www.w3.org/2002/07/owl#Restriction",
        "http://www.w3.org/2002/07/owl#Class",
        "http://www.w3.org/2000/01/rdf-schema#Datatype",
        "http://www.w3.org/2000/01/rdf-schema#Class",
    ];

    pub fn is_annotation(predicate: &str) -> bool {
        Self::PREDICATES.contains(&predicate)
    }

    pub fn is_expression_type(kind: &str) -> bool {
        Self::TYPES.contains(&kind)
    }
}
