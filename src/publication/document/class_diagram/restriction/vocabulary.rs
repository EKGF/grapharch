//! The OWL restriction vocabulary this mapping recognises.
pub(super) const SUBCLASS: &str =
    "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// One OWL predicate of a restriction node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Term {
    OnProperty,
    OnClass,
    OnDataRange,
    SomeValuesFrom,
    AllValuesFrom,
    HasValue,
    MinCardinality,
    Cardinality,
    MaxCardinality,
    MinQualifiedCardinality,
    QualifiedCardinality,
    MaxQualifiedCardinality,
}
/// Which bound a cardinality predicate states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bound {
    Minimum,
    Exact,
    Maximum,
}
impl Term {
    const ALL: [Self; 12] = [
        Self::OnProperty,
        Self::OnClass,
        Self::OnDataRange,
        Self::SomeValuesFrom,
        Self::AllValuesFrom,
        Self::HasValue,
        Self::MinCardinality,
        Self::Cardinality,
        Self::MaxCardinality,
        Self::MinQualifiedCardinality,
        Self::QualifiedCardinality,
        Self::MaxQualifiedCardinality,
    ];

    /// The recognised term for a predicate IRI, if any.
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
            Self::MinCardinality => "minCardinality",
            Self::Cardinality => "cardinality",
            Self::MaxCardinality => "maxCardinality",
            Self::MinQualifiedCardinality => "minQualifiedCardinality",
            Self::QualifiedCardinality => "qualifiedCardinality",
            Self::MaxQualifiedCardinality => "maxQualifiedCardinality",
        }
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
