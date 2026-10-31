use {
    crate::publication::{RDF_TYPE, dataset::identity, term},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};

pub(super) fn q(
    subject: &NamedNode,
    field: &str,
    object: impl Into<Term>,
) -> Quad {
    Quad::new(
        subject.clone(),
        if field == "type" {
            NamedNode::new_unchecked(RDF_TYPE)
        } else {
            term(field)
        },
        object,
        GraphName::DefaultGraph,
    )
}
pub(super) fn placement(
    quads: &mut Vec<Quad>,
    target: &NamedNode,
    profile: &NamedNode,
    component: &NamedNode,
    title: &str,
    kind: &str,
    order: u32,
) {
    let placement = identity("ontology-placement", &[
        target.as_str(),
        component.as_str(),
    ]);
    quads.extend([
        q(profile, "placement", placement.clone()),
        q(&placement, "type", term("Placement")),
        q(&placement, "component", component.clone()),
        q(&placement, "order", Literal::from(order)),
        q(
            &placement,
            "sectionKind",
            Literal::new_simple_literal(kind),
        ),
        q(component, "type", term("Component")),
    ]);
    quads.push(Quad::new(
        component.clone(),
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        Literal::new_simple_literal(title),
        GraphName::DefaultGraph,
    ));
}

pub(super) fn prefixes(
    namespace: &[String],
) -> std::sync::Arc<std::collections::BTreeMap<String, String>> {
    std::sync::Arc::new(std::collections::BTreeMap::from([
        (
            "https://ekgf.org/ontology/artifact-governance#".into(),
            "artgov".into(),
        ),
        (
            "https://www.omg.org/spec/Commons/PartiesAndSituations/".into(),
            "cmns-pts".into(),
        ),
        (namespace[0].clone(), namespace[1].clone()),
        (
            "http://www.w3.org/2002/07/owl#".into(),
            "owl".into(),
        ),
        (
            "http://www.w3.org/2000/01/rdf-schema#".into(),
            "rdfs".into(),
        ),
        (
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#".into(),
            "rdf".into(),
        ),
        (
            "http://www.w3.org/2001/XMLSchema#".into(),
            "xsd".into(),
        ),
        ("http://purl.org/dc/terms/".into(), "dct".into()),
        ("http://www.w3.org/ns/prov#".into(), "prov".into()),
        (
            "http://www.w3.org/2004/02/skos/core#".into(),
            "skos".into(),
        ),
    ]))
}
