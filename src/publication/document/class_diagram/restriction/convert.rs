//! Map one typed OWL restriction to the UML notation that states it.
//!
//! A restriction maps when its property is a named property or the inverse
//! of one and its filler is a named class or datatype, a union of named
//! classes (one exclusive end per member), a nested existential chain (a
//! path member `p.q`), or an expression every individual satisfies. Every
//! other restriction is stated by an OCL-style constraint note instead.
use {
    super::form::{Constraint, Filler, Restriction},
    crate::publication::document::class_diagram::{
        Node,
        NodeKind,
        expression::{
            Bound,
            ClassExpression,
            PropertyExpression,
            Quantifier,
            RestrictionExpression,
        },
        multiplicity::Cardinality,
        shorthand::GENERIC_RESOURCE,
    },
};

impl<'a> Filler<'a> {
    fn named(node: &'a Node) -> Option<Self> {
        (node.kind.is_class_like() && node.id != GENERIC_RESOURCE)
            .then_some(Self { node, data: node.datatype })
    }

    /// Named fillers of a named class or a union of named classes.
    fn alternatives(expression: &'a ClassExpression) -> Option<Vec<Self>> {
        match expression {
            ClassExpression::Named(node) => Some(vec![Self::named(node)?]),
            ClassExpression::Union(parts) => {
                parts
                    .iter()
                    .map(|part| Self::named(part.named()?).filter(|f| !f.data))
                    .collect() // bounded: MEMBERS.
            },
            _ => None,
        }
    }
}
impl Bound {
    fn cardinality(self, count: u64) -> Cardinality {
        match self {
            Self::Minimum => Cardinality::at_least(count),
            Self::Exact => Cardinality::exactly(count),
            Self::Maximum => Cardinality::at_most(count),
        }
    }
}
impl<'a> Restriction<'a> {
    /// The notation of one restriction on `class`, one restriction per
    /// exclusive end, or `None` when only a constraint note states it.
    pub fn from_expression(
        class: &'a Node,
        expression: &'a RestrictionExpression,
        defining: bool,
    ) -> Option<Vec<Self>> {
        let (property, inverse) = match &expression.property {
            PropertyExpression::Named(p) => (p, false),
            PropertyExpression::Inverse(p) => (p, true),
        };
        if !matches!(
            property.kind,
            NodeKind::ObjectProperty |
                NodeKind::DatatypeProperty |
                NodeKind::RdfProperty |
                NodeKind::Resource
        ) || (inverse && property.kind == NodeKind::DatatypeProperty)
        {
            return None;
        }
        let one = |constraint, path: Vec<&'a Node>| {
            Self {
                class,
                property,
                path,
                inverse,
                constraint,
                defining,
                alternative: false,
            }
        };
        let each =
            |fillers: Vec<Filler<'a>>,
             make: &dyn Fn(Filler<'a>) -> Constraint<'a>| {
                let alternative = fillers.len() > 1;
                fillers
                    .into_iter()
                    .filter(|f| Self::compatible(property, inverse, f))
                    .map(|filler| {
                        Self { alternative, ..one(make(filler), Vec::new()) }
                    })
                    .collect::<Vec<_>>() // bounded: MEMBERS.
            };
        let drawn = match &expression.quantifier {
            Quantifier::Some(filler) if filler.is_top() => {
                vec![one(Constraint::Exists, Vec::new())]
            },
            Quantifier::Some(filler) => {
                match Filler::alternatives(filler) {
                    Some(fillers) => each(fillers, &Constraint::Some),
                    None if !inverse => {
                        let (path, filler) = Self::chain(filler)?;
                        vec![one(Constraint::Some(filler), path)]
                    },
                    None => return None,
                }
            },
            Quantifier::All(filler) => {
                each(Filler::alternatives(filler)?, &Constraint::All)
            },
            Quantifier::Value(value) => {
                let compatible = if value.kind == NodeKind::LiteralValue {
                    property.kind != NodeKind::ObjectProperty && !inverse
                } else {
                    property.kind != NodeKind::DatatypeProperty &&
                        value.kind.is_class_like()
                };
                if !compatible {
                    return None;
                }
                vec![one(Constraint::Value(value), Vec::new())]
            },
            Quantifier::Myself => return None,
            Quantifier::Count { bound, count, filler } => {
                let cardinality = bound.cardinality(*count);
                // `{no p}` names a property of the class, never its inverse.
                if inverse && cardinality == Cardinality::Prohibited {
                    return None;
                }
                match filler {
                    None => {
                        vec![one(
                            Constraint::Count { cardinality, filler: None },
                            Vec::new(),
                        )]
                    },
                    Some(filler) if filler.is_top() => {
                        vec![one(
                            Constraint::Count { cardinality, filler: None },
                            Vec::new(),
                        )]
                    },
                    Some(filler) => {
                        each(Filler::alternatives(filler)?, &|f| {
                            Constraint::Count { cardinality, filler: Some(f) }
                        })
                    },
                }
            },
        };
        (!drawn.is_empty() && drawn.len() == Self::expected(expression))
            .then_some(drawn)
    }

    /// How many ends a fully compatible restriction draws.
    fn expected(expression: &RestrictionExpression) -> usize {
        let filler = match &expression.quantifier {
            Quantifier::Some(filler) | Quantifier::All(filler) => Some(filler),
            Quantifier::Count { filler, .. } => filler.as_ref(),
            _ => None,
        };
        match filler {
            Some(ClassExpression::Union(parts)) if !matches!(&expression.quantifier, Quantifier::Some(f) if f.is_top()) => {
                parts.len()
            },
            _ => 1,
        }
    }

    /// Whether the filler suits the property: data values for datatype
    /// properties, individuals for object properties. An undeclared
    /// property takes the kind of its filler.
    fn compatible(property: &Node, inverse: bool, filler: &Filler) -> bool {
        if filler.data {
            property.kind != NodeKind::ObjectProperty && !inverse
        } else {
            property.kind != NodeKind::DatatypeProperty
        }
    }

    /// A chain of existentials `q some (r some C)`, as the further path
    /// properties and the final named filler.
    fn chain(
        expression: &'a ClassExpression,
    ) -> Option<(Vec<&'a Node>, Filler<'a>)> {
        let ClassExpression::Restriction(inner) = expression else {
            return None;
        };
        let PropertyExpression::Named(property) = &inner.property else {
            return None;
        };
        let filler = match &inner.quantifier {
            Quantifier::Some(filler) => filler,
            Quantifier::Count {
                bound: Bound::Minimum | Bound::Exact,
                count: 1..,
                filler: Some(filler),
            } if matches!(inner.quantifier, Quantifier::Count {
                bound: Bound::Minimum,
                ..
            }) =>
            {
                filler
            },
            _ => return None,
        };
        let (mut path, named) = match Filler::alternatives(filler) {
            Some(fillers) if fillers.len() == 1 => (Vec::new(), fillers[0]),
            Some(_) => return None,
            None => Self::chain(filler)?,
        };
        if !Self::compatible(property, false, &named) && path.is_empty() {
            return None;
        }
        path.insert(0, property);
        Some((path, named))
    }
}
