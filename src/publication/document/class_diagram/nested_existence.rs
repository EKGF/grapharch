//! A single proven tautological filler; never discard the outer existential.
use {
    super::{
        Diagram,
        Node,
        NodeKind,
        escape::Escaped,
        restriction::{Notation, Restriction},
    },
    std::collections::BTreeMap,
};
const SOME: &str = "http://www.w3.org/2002/07/owl#someValuesFrom";
const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
const ON_CLASS: &str = "http://www.w3.org/2002/07/owl#onClass";
const MINIMUM: &str = "http://www.w3.org/2002/07/owl#minQualifiedCardinality";
pub(super) struct Existence<'a> {
    pub class:    &'a Node,
    property:     &'a Node,
    pub consumed: Vec<usize>,
}
impl Existence<'_> {
    pub fn covered(&self, counts: &[(Restriction<'_>, Notation<'_>)]) -> bool {
        counts.iter().any(|(count, _)| {
            count.class.id == self.class.id &&
                count.property.id == self.property.id &&
                count.guarantees_existence()
        })
    }

    pub fn write(
        &self,
        output: &mut String,
        identifiers: &BTreeMap<&str, usize>,
        counts: &[(Restriction<'_>, Notation<'_>)],
    ) {
        if !self.covered(counts) {
            output.push_str(&format!(
                "  {{field}} {} [1..*] [[[./grapharch-node/n{}]]]\n",
                Escaped::inline(&self.property.label),
                identifiers[self.property.id.as_str()]
            ));
        }
    }
}
pub(super) fn collect(diagram: &Diagram) -> Vec<Existence<'_>> {
    diagram
        .edges
        .iter()
        .filter_map(|edge| {
            if edge.predicate.as_deref() != Some(SUBCLASS) ||
                edge.from.kind != NodeKind::Class ||
                edge.to.kind != NodeKind::OwlExpression
            {
                return None;
            }
            let outer: Vec<_> = diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.from.id == edge.to.id || e.to.id == edge.to.id
                })
                .collect(); // bounded: eighteen retained facts.
            if outer.len() != 3 {
                return None;
            }
            let property = outer
                .iter()
                .find(|(_, e)| {
                    e.from.id == edge.to.id &&
                        e.predicate.as_deref() == Some(ON_PROPERTY)
                })?
                .1;
            let filler = outer
                .iter()
                .find(|(_, e)| {
                    e.from.id == edge.to.id &&
                        e.predicate.as_deref() == Some(SOME)
                })?
                .1;
            if !matches!(
                property.to.kind,
                NodeKind::ObjectProperty |
                    NodeKind::Resource |
                    NodeKind::RdfProperty
            ) || filler.to.kind != NodeKind::OwlExpression
            {
                return None;
            }
            let inner: Vec<_> = diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.from.id == filler.to.id || e.to.id == filler.to.id
                })
                .collect(); // bounded: eighteen retained facts.
            if inner.len() != 4 {
                return None;
            }
            let target = |predicate| {
                inner
                    .iter()
                    .find(|(_, e)| {
                        e.from.id == filler.to.id &&
                            e.predicate.as_deref() == Some(predicate)
                    })
                    .map(|(_, e)| &e.to)
            };
            if !matches!(
                target(ON_PROPERTY)?.kind,
                NodeKind::ObjectProperty |
                    NodeKind::Resource |
                    NodeKind::RdfProperty
            ) || !matches!(
                target(ON_CLASS)?.kind,
                NodeKind::Class | NodeKind::Resource
            ) || target(MINIMUM)?.count()? != 0
            {
                return None;
            }
            Some(Existence {
                class:    &edge.from,
                property: &property.to,
                consumed: outer
                    .iter()
                    .chain(inner.iter())
                    .map(|(i, _)| *i)
                    .collect(),
            }) // bounded: six unique facts plus the shared edge.
        })
        .collect() // bounded: eighteen retained facts.
}
