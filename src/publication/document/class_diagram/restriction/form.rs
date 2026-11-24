//! The typed meaning of one OWL restriction that UML notation can state.
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
    /// At least one value, of any type: an existential whose filler every
    /// individual satisfies.
    Exists,
}
/// One restriction on one class, drawn as a member, association or class
/// constraint.
#[derive(Clone, Debug)]
pub(in super::super) struct Restriction<'a> {
    pub class:       &'a Node,
    pub property:    &'a Node,
    /// Further properties of a path member `p.q`, outermost first.
    pub path:        Vec<&'a Node>, // bounded: the expression nesting depth.
    /// The restricted property is the inverse of `property`.
    pub inverse:     bool,
    pub constraint:  Constraint<'a>,
    /// The restriction is part of an equivalence: it defines the class.
    pub defining:    bool,
    /// One member of a union filler, drawn as one of exclusive ends.
    pub alternative: bool,
}
impl Restriction<'_> {
    /// Whether the restriction requires at least one individual value.
    pub fn guarantees_existence(&self) -> bool {
        match self.constraint {
            Constraint::Count { cardinality, filler } => {
                !self.alternative &&
                    self.path.is_empty() &&
                    filler.is_none_or(|f| !f.data) &&
                    cardinality
                        .multiplicity()
                        .is_some_and(|m| m.requires_value())
            },
            _ => false,
        }
    }
}
