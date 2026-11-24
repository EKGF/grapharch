//! The UML notation of one simple restriction.
//!
//! | Restriction | Datatype values | Individuals |
//! | --- | --- | --- |
//! | unqualified count | member `p : range [m]` | association to the single declared range, else member `p [m]` |
//! | qualified count | member `p : D [m] {qualified}` | association `m` to `C` with `{qualified}` |
//! | `someValuesFrom` | member `p : D [1..*] {qualified}` | association `1..*` to `C` with `{qualified}` |
//! | `allValuesFrom` | member `p : D {all values}` | association `*` to `C` with `{all values}` |
//! | `hasValue` | member `p {= v}` | association to the individual with `{= v}` |
//! | maximum or exact zero | class constraint `{no p}` (`{no p in C}` when qualified) | the same constraint |
//!
//! A restriction on a property whose declared domain is an asserted
//! superclass adds `{redefines}`; one stated by an equivalence adds
//! `{defining}`; each member of a union filler is its own end with `{xor}`.
//! A nested existential chain is the path member `p.q : C [1..*]`, and a
//! restriction on an inverse property is an association from the filler.
//! Neighbouring classes draw only members, so the focus neighbourhood stays
//! bounded.
use {
    super::form::{Constraint, Filler, Restriction},
    crate::publication::document::class_diagram::{
        Node,
        context::Context,
        multiplicity::{Cardinality, Multiplicity},
        shorthand::GENERIC_RESOURCE,
    },
};

/// Whether the restricted class is the documented class or a neighbour.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Scope {
    Focus,
    Neighbour,
}
/// How one restriction is drawn.
pub(in super::super) enum Notation<'a> {
    /// The restriction imposes nothing, such as a zero minimum.
    Hidden,
    Member {
        /// The member name: a property label or a path `p.q`.
        name:         String,
        property:     &'a Node,
        types:        Vec<&'a Node>, // bounded: four declared ranges.
        multiplicity: Option<Multiplicity>,
        tags:         Vec<String>, // bounded: five qualifiers.
    },
    Association {
        property:     &'a Node,
        target:       &'a Node,
        multiplicity: Option<Multiplicity>,
        tags:         Vec<String>, // bounded: five qualifiers.
        /// The association runs from the target, for an inverse property.
        reversed:     bool,
    },
    /// No value, or no value in the named filler, may exist.
    Prohibition { property: &'a Node, filler: Option<&'a Node> },
}
impl<'a> Restriction<'a> {
    pub fn notation(&self, context: &'a Context, scope: Scope) -> Notation<'a> {
        let usage = context.property_use(&self.class.id, &self.property.id);
        let mut tags = Vec::new(); // bounded: five qualifiers.
        let (filler, multiplicity) = match self.constraint {
            Constraint::Count { cardinality, filler } => {
                let Some(multiplicity) = cardinality.multiplicity() else {
                    return Notation::Prohibition {
                        property: self.property,
                        filler:   filler.map(|f| f.node),
                    };
                };
                if multiplicity.is_unconstrained() {
                    return Notation::Hidden;
                }
                if filler.is_some() {
                    tags.push("qualified".to_owned());
                }
                (filler, Some(multiplicity))
            },
            Constraint::Some(filler) => {
                tags.push("qualified".to_owned());
                (
                    Some(filler),
                    Cardinality::at_least(1).multiplicity(),
                )
            },
            Constraint::All(filler) => {
                tags.push("all values".to_owned());
                (
                    Some(filler),
                    (!filler.data)
                        .then(|| Cardinality::at_least(0).multiplicity())
                        .flatten(),
                )
            },
            Constraint::Exists => {
                (None, Cardinality::at_least(1).multiplicity())
            },
            Constraint::Value(value) => {
                tags.push(format!("= {}", value.label));
                (
                    (value.kind.is_class_like())
                        .then_some(Filler { node: value, data: false }),
                    None,
                )
            },
        };
        if usage.is_some_and(|u| u.inherited) && !self.inverse {
            tags.push("redefines".to_owned());
        }
        if self.alternative {
            tags.push("xor".to_owned());
        }
        if self.defining {
            tags.push("defining".to_owned());
        }
        let ranges: Vec<&Node> =
            usage.map(|u| u.ranges.iter().collect()).unwrap_or_default(); // bounded: four declared ranges.
        let target = match filler {
            Some(filler) if !filler.data => Some(filler.node),
            Some(_) => None,
            None if self.inverse ||
                matches!(self.constraint, Constraint::Exists) =>
            {
                None
            },
            // An unqualified count reaches the single declared class range.
            None => {
                match ranges.as_slice() {
                    [range]
                        if !self.property.kind.is_data() &&
                            !range.datatype &&
                            range.kind.is_class_like() &&
                            range.id != GENERIC_RESOURCE =>
                    {
                        Some(*range)
                    },
                    _ => None,
                }
            },
        };
        let name = self.name();
        match (target, scope, self.constraint) {
            (Some(target), Scope::Focus, _) if self.path.is_empty() => {
                Notation::Association {
                    property: self.property,
                    target,
                    multiplicity,
                    tags,
                    reversed: self.inverse,
                }
            },
            (_, _, Constraint::Value(_) | Constraint::Exists) => {
                Notation::Member {
                    name,
                    property: self.property,
                    types: Vec::new(),
                    multiplicity,
                    tags,
                }
            },
            (target, ..) => {
                Notation::Member {
                    name,
                    property: self.property,
                    types: match (target, filler) {
                        (Some(target), _) => vec![target],
                        (None, Some(filler)) => vec![filler.node],
                        (None, None) => ranges,
                    },
                    multiplicity,
                    tags,
                }
            },
        }
    }
}
impl Restriction<'_> {
    /// The member name: the property, `inverse of` it, or a path `p.q`.
    fn name(&self) -> String {
        let first = if self.inverse {
            format!("inverse of {}", self.property.label)
        } else {
            self.property.label.clone()
        };
        std::iter::once(first)
            .chain(self.path.iter().map(|p| p.label.clone()))
            .collect::<Vec<_>>() // bounded: the expression nesting depth.
            .join(".")
    }
}
