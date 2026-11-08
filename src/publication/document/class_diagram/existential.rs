//! Class-scoped existential shorthand, never a global property range or count.
use super::{Diagram, Node};

const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
const SOME_VALUES: &str = "http://www.w3.org/2002/07/owl#someValuesFrom";

pub(super) struct Existential<'a> {
    pub class:    &'a Node,
    pub property: &'a Node,
    pub filler:   &'a Node,
    pub consumed: Vec<usize>,
}

pub(super) fn collect(diagram: &Diagram) -> Vec<Existential<'_>> {
    diagram.edges.iter().filter_map(|edge| {
        if edge.predicate.as_deref() != Some(SUBCLASS) ||
            edge.from.kind != "Class" ||
            edge.to.kind != "OWL expression"
        {
            return None;
        }
        // The reader retains every OWL predicate on a declared restriction.
        // Additional qualifiers therefore prevent this narrow simplification.
        let incident: Vec<_> = diagram.edges.iter().enumerate()
            .filter(|(_, candidate)| candidate.from.id == edge.to.id || candidate.to.id == edge.to.id)
            .collect(); // bounded: eighteen retained facts.
        if incident.len() != 3 { return None; }
        let property = incident.iter().find(|(_, e)| e.from.id == edge.to.id && e.predicate.as_deref() == Some(ON_PROPERTY))?.1;
        let filler = incident.iter().find(|(_, e)| e.from.id == edge.to.id && e.predicate.as_deref() == Some(SOME_VALUES))?.1;
        if !matches!(property.to.kind.as_str(), "Object property" | "RDF property" | "Resource") ||
            !matches!(filler.to.kind.as_str(), "Class" | "Resource") ||
            filler.to.id == super::shorthand::GENERIC_RESOURCE
        { return None; }
        Some(Existential {
            class: &edge.from,
            property: &property.to,
            filler: &filler.to,
            consumed: incident.iter().map(|(index, _)| *index).collect(), // bounded: three complete facts.
        })
    }).collect() // bounded: eighteen retained facts.
}
