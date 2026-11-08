//! Class-scoped qualified minima; never a global property range or count.
use {
    super::{Diagram, Node, layout::Layout, projection::label},
    std::collections::BTreeMap,
};

const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
const ON_CLASS: &str = "http://www.w3.org/2002/07/owl#onClass";
const MINIMUM: &str = "http://www.w3.org/2002/07/owl#minQualifiedCardinality";

pub(super) struct QualifiedMinimum<'a> {
    pub class:    &'a Node,
    pub property: &'a Node,
    pub filler:   &'a Node,
    pub minimum:  u64,
    pub consumed: Vec<usize>,
}
impl QualifiedMinimum<'_> {
    pub fn write(
        &self,
        output: &mut String,
        identifiers: &BTreeMap<&str, usize>,
        layout: &Layout<'_>,
    ) {
        if self.minimum == 0 {
            return;
        }
        output.push_str(&format!(
            "n{} {} \"{}..*\" n{} [[./grapharch-node/n{}]] : \
             {}\\n{{qualified}}\n",
            identifiers[self.class.id.as_str()],
            layout.association_arrow(&self.class.id, &self.filler.id),
            self.minimum,
            identifiers[self.filler.id.as_str()],
            identifiers[self.property.id.as_str()],
            label(&self.property.label),
        ));
    }
}

pub(super) fn collect(diagram: &Diagram) -> Vec<QualifiedMinimum<'_>> {
    diagram
        .edges
        .iter()
        .filter_map(|edge| {
            if edge.predicate.as_deref() != Some(SUBCLASS) ||
                edge.from.kind != "Class" ||
                edge.to.kind != "OWL expression"
            {
                return None;
            }
            let incident: Vec<_> = diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate.from.id == edge.to.id ||
                        candidate.to.id == edge.to.id
                })
                .collect(); // bounded: eighteen retained facts.
            // A complete direct subclass restriction only: extra predicates,
            // shared expression nodes or boolean nesting prevent this
            // simplification.
            if incident.len() != 4 {
                return None;
            }
            let target = |predicate| {
                incident
                    .iter()
                    .find(|(_, e)| {
                        e.from.id == edge.to.id &&
                            e.predicate.as_deref() == Some(predicate)
                    })
                    .map(|(_, e)| &e.to)
            };
            let property = target(ON_PROPERTY)?;
            let filler = target(ON_CLASS)?;
            let count = target(MINIMUM)?;
            if !matches!(
                property.kind.as_str(),
                "Object property" | "RDF property" | "Resource"
            ) || !matches!(filler.kind.as_str(), "Class" | "Resource") ||
                count.kind != "Literal value" ||
                filler.id == super::shorthand::GENERIC_RESOURCE
            {
                return None;
            }
            // Read the original typed RDF literal, never its display label.
            let literal = count.id.parse::<oxrdf::Literal>().ok()?;
            if !matches!(
                literal.datatype().as_str(),
                "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" |
                    "http://www.w3.org/2001/XMLSchema#integer"
            ) {
                return None;
            }
            let minimum = literal.value().parse::<u64>().ok()?;
            Some(QualifiedMinimum {
                class: &edge.from,
                property,
                filler,
                minimum,
                consumed: incident.iter().map(|(i, _)| *i).collect(),
            }) // bounded: four complete facts.
        })
        .collect() // bounded: eighteen retained facts.
}
