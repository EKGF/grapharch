//! RDF datatypes, recognised by declaration or as OWL 2 built-in datatypes.

pub(super) const RDFS_DATATYPE: &str =
    "http://www.w3.org/2000/01/rdf-schema#Datatype";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const RDFS_LITERAL: &str = "http://www.w3.org/2000/01/rdf-schema#Literal";
const OWL_DATATYPES: [&str; 2] = [
    "http://www.w3.org/2002/07/owl#real",
    "http://www.w3.org/2002/07/owl#rational",
];
const RDF_DATATYPES: [&str; 6] = [
    "langString",
    "dirLangString",
    "PlainLiteral",
    "XMLLiteral",
    "HTML",
    "JSON",
];

/// Whether an IRI names a datatype of the OWL 2 datatype map or RDF 1.2,
/// which need no `rdfs:Datatype` declaration in the selected sources.
pub(super) fn is_builtin(iri: &str) -> bool {
    iri.strip_prefix(XSD).is_some_and(|local| !local.is_empty()) ||
        iri.strip_prefix(RDF)
            .is_some_and(|local| RDF_DATATYPES.contains(&local)) ||
        iri == RDFS_LITERAL ||
        OWL_DATATYPES.contains(&iri)
}

#[cfg(test)]
mod tests {
    #[test]
    fn built_in_datatypes_are_recognised_without_declarations() {
        for iri in [
            "http://www.w3.org/2001/XMLSchema#string",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString",
            "http://www.w3.org/2000/01/rdf-schema#Literal",
            "http://www.w3.org/2002/07/owl#real",
        ] {
            assert!(super::is_builtin(iri), "{iri}");
        }
        for iri in [
            "http://www.w3.org/2001/XMLSchema#",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property",
            "http://www.w3.org/2002/07/owl#Thing",
        ] {
            assert!(!super::is_builtin(iri), "{iri}");
        }
    }
}
