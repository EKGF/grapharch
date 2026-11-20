//! The typed meaning of one simple, direct OWL class restriction.
use super::super::{Node, multiplicity::Cardinality};

/// A named restriction filler: a class or a datatype.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Filler<'a> {
    pub node: &'a Node,
    /// Values are data values rather than individuals.
    pub data: bool,
}
/// What a restriction requires of the restricted property's values.
#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Constraint<'a> {
    /// A cardinality, qualified by a filler when one is named.
    Count {
        cardinality: Cardinality,
        filler:      Option<Filler<'a>>,
    },
    /// `owl:someValuesFrom`: at least one value in the filler.
    Some(Filler<'a>),
    /// `owl:allValuesFrom`: every value is in the filler.
    All(Filler<'a>),
    /// `owl:hasValue`: this individual or literal is among the values.
    Value(&'a Node),
}
/// One `rdfs:subClassOf` restriction with its complete retained facts.
#[derive(Clone, Debug)]
pub(in super::super) struct Restriction<'a> {
    pub class:      &'a Node,
    pub property:   &'a Node,
    pub constraint: Constraint<'a>,
    /// Indices of every retained fact the notation replaces.
    pub consumed:   Vec<usize>, // bounded: four facts of one restriction.
}
impl Restriction<'_> {
    /// Whether the restriction requires at least one individual value.
    pub fn guarantees_existence(&self) -> bool {
        match self.constraint {
            Constraint::Count { cardinality, filler } => {
                filler.is_none_or(|f| !f.data) &&
                    cardinality
                        .multiplicity()
                        .is_some_and(|m| m.requires_value())
            },
            _ => false,
        }
    }
}
