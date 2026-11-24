//! RDF and OWL classes of classes and properties, drawn as UML metaclasses.

/// A UML metaclass that an RDF vocabulary term names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Metaclass {
    Class,
    Property,
}
impl Metaclass {
    /// The metaclass an IRI names: `rdfs:Class` and `owl:Class` are Class,
    /// `rdf:Property` is Property.
    pub fn of(iri: &str) -> Option<Self> {
        match iri {
            "http://www.w3.org/2000/01/rdf-schema#Class" |
            "http://www.w3.org/2002/07/owl#Class" => Some(Self::Class),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property" => {
                Some(Self::Property)
            },
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Property => "Property",
        }
    }
}
