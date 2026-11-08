//! Conservative presentation shorthand; original assertions remain unchanged.
use {
    super::{Diagram, Node},
    std::collections::BTreeSet,
};
const DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
pub(super) const GENERIC_RESOURCE: &str =
    "http://www.w3.org/2000/01/rdf-schema#Resource";
const RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";

pub(super) struct Property<'a> {
    pub node:   &'a Node,
    pub domain: &'a Node,
    pub range:  Option<&'a Node>,
}
impl Property<'_> {
    pub fn attribute(&self) -> bool {
        self.node.kind == "Datatype property" ||
            self.range.is_none() ||
            self.range.is_some_and(|range| range.id == GENERIC_RESOURCE)
    }
}
pub(super) struct Shorthand<'a> {
    pub properties:   Vec<Property<'a>>,
    pub existentials: Vec<super::existential::Existential<'a>>,
    pub consumed:     BTreeSet<usize>,
    pub visible:      BTreeSet<&'a str>,
}
impl<'a> Shorthand<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let mut properties = Vec::new(); // bounded: eighteen retained edges.
        let mut consumed = BTreeSet::new(); // bounded: eighteen edge indices.
        let mut visited = BTreeSet::new(); // bounded: thirty-seven nodes.
        for node in diagram.nodes() {
            if !visited.insert(node.id.as_str()) ||
                !matches!(
                    node.kind.as_str(),
                    "Object property" |
                        "Datatype property" |
                        "Annotation property" |
                        "RDF property" |
                        "Resource"
                )
            {
                continue;
            }
            let incident: Vec<_> = diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| e.from.id == node.id || e.to.id == node.id)
                .collect(); // bounded: eighteen edges.
            // A property referenced by an expression/other relationship stays
            // explicit, so no disappearing node changes that relationship.
            if incident.iter().any(|(_, e)| {
                e.from.id != node.id ||
                    !matches!(e.predicate.as_deref(), Some(DOMAIN | RANGE))
            }) {
                continue;
            }
            let domains: Vec<_> = incident
                .iter()
                .filter(|(_, e)| e.predicate.as_deref() == Some(DOMAIN))
                .map(|(_, e)| &e.to)
                .collect(); // bounded: eighteen edges.
            let ranges: Vec<_> = incident
                .iter()
                .filter(|(_, e)| e.predicate.as_deref() == Some(RANGE))
                .map(|(_, e)| &e.to)
                .collect(); // bounded: eighteen edges.
            if domains.len() != 1 ||
                domains[0].kind != "Class" ||
                domains[0].id == GENERIC_RESOURCE ||
                ranges.len() > 1
            {
                continue;
            }
            let range = ranges.first().copied();
            if range.is_some_and(|n| {
                !matches!(n.kind.as_str(), "Class" | "Resource")
            }) {
                continue;
            }
            consumed.extend(incident.iter().map(|(i, _)| *i));
            properties.push(Property { node, domain: domains[0], range });
        }
        let existentials = super::existential::collect(diagram);
        for existential in &existentials {
            consumed.extend(&existential.consumed);
        }
        // Suppress only the generic RDF root in this presentation, never facts.
        consumed.extend(
            diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.from.id == GENERIC_RESOURCE ||
                        edge.to.id == GENERIC_RESOURCE
                })
                .map(|(index, _)| index),
        );
        let mut visible = BTreeSet::from([diagram.focus.id.as_str()]); // bounded: thirty-seven nodes.
        for (index, edge) in diagram.edges.iter().enumerate() {
            if !consumed.contains(&index) {
                visible.extend([edge.from.id.as_str(), edge.to.id.as_str()]);
            }
        }
        for property in &properties {
            visible.insert(property.domain.id.as_str());
            if !property.attribute() &&
                let Some(range) = property.range
            {
                visible.insert(range.id.as_str());
            }
        }
        for existential in &existentials {
            visible.extend([
                existential.class.id.as_str(),
                existential.filler.id.as_str(),
            ]);
        }
        visible.remove(GENERIC_RESOURCE);
        Self { properties, existentials, consumed, visible }
    }
}
