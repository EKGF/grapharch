//! OCL-style plain text for expressions that no UML element states.
use super::model::{
    AxiomRelation,
    Bound,
    ClassAxiom,
    ClassExpression,
    PropertyExpression,
    Quantifier,
    RestrictionExpression,
};

/// Iterator names by nesting depth; deeper levels reuse the last one with an
/// ordinal.
const VARIABLES: [&str; 4] = ["x", "y", "z", "w"];

/// One rendered fragment and whether it needs parentheses when combined.
pub(in super::super) struct Ocl {
    pub text: String,
    compound: bool,
}
impl Ocl {
    fn atom(text: String) -> Self { Self { text, compound: false } }

    fn nested(self) -> String {
        if self.compound {
            format!("({})", self.text)
        } else {
            self.text
        }
    }

    fn variable(depth: usize) -> String {
        VARIABLES.get(depth).map_or_else(
            || format!("w{}", depth + 1 - VARIABLES.len()),
            |name| (*name).to_owned(),
        )
    }
}
impl ClassExpression {
    /// The condition this expression states of `subject`.
    pub(in super::super) fn ocl(&self, subject: &str, depth: usize) -> Ocl {
        match self {
            Self::Named(_) if self.is_top() => Ocl::atom("true".into()),
            Self::Named(node) => {
                Ocl::atom(format!(
                    "{subject}.oclIsKindOf({})",
                    node.type_label()
                ))
            },
            Self::Restriction(restriction) => restriction.ocl(subject, depth),
            Self::Union(parts) | Self::Intersection(parts) => {
                let operator = if matches!(self, Self::Union(_)) {
                    " or "
                } else {
                    " and "
                };
                Ocl {
                    text:     parts
                        .iter()
                        .map(|p| p.ocl(subject, depth).nested())
                        .collect::<Vec<_>>() // bounded: MEMBERS.
                        .join(operator),
                    compound: true,
                }
            },
            Self::Complement(inner) => {
                Ocl::atom(format!(
                    "not {}",
                    inner.ocl(subject, depth).nested()
                ))
            },
            Self::OneOf(values) => {
                Ocl::atom(format!(
                    "Set{{{}}}->includes({subject})",
                    values
                        .iter()
                        .map(|v| v.label.as_str())
                        .collect::<Vec<_>>() // bounded: MEMBERS.
                        .join(", ")
                ))
            },
        }
    }
}
impl RestrictionExpression {
    fn ocl(&self, subject: &str, depth: usize) -> Ocl {
        let navigation = match &self.property {
            PropertyExpression::Named(p) => format!("{subject}.{}", p.label),
            PropertyExpression::Inverse(p) => {
                format!("{subject}.inverse({})", p.label)
            },
        };
        let variable = Ocl::variable(depth);
        let condition = |filler: &ClassExpression| {
            format!(
                "{variable} | {}",
                filler.ocl(&variable, depth + 1).text
            )
        };
        Ocl::atom(match &self.quantifier {
            Quantifier::Some(filler) => {
                format!("{navigation}->exists({})", condition(filler))
            },
            Quantifier::All(filler) => {
                format!("{navigation}->forAll({})", condition(filler))
            },
            Quantifier::Value(value) => {
                format!("{navigation}->includes({})", value.label)
            },
            Quantifier::Myself => format!("{navigation}->includes({subject})"),
            Quantifier::Count { bound, count, filler } => {
                let values = match filler {
                    Some(filler) => {
                        format!("{navigation}->select({})", condition(filler))
                    },
                    None => navigation,
                };
                let operator = match bound {
                    Bound::Minimum => ">=",
                    Bound::Exact => "=",
                    Bound::Maximum => "<=",
                };
                format!("{values}->size() {operator} {count}")
            },
        })
    }
}
impl ClassAxiom {
    /// The constraint note stating this axiom, or the part of it no other
    /// notation draws.
    pub fn note(&self, expression: &ClassExpression) -> String {
        let defining = if self.relation == AxiomRelation::Equivalent {
            "{defining} "
        } else {
            ""
        };
        if let ClassExpression::Complement(inner) = expression &&
            self.relation != AxiomRelation::Superclass
        {
            let negated = match inner.named() {
                Some(node) => node.type_label(),
                None => inner.ocl("self", 0).nested(),
            };
            return format!("{defining}{{not {negated}}}");
        }
        let condition = expression.ocl("self", 0);
        match self.relation {
            AxiomRelation::Superclass => {
                format!(
                    "inv: {} implies self.oclIsKindOf({})",
                    condition.nested(),
                    self.class.type_label()
                )
            },
            AxiomRelation::Disjoint => {
                format!("inv: not {}", condition.nested())
            },
            _ => format!("{defining}inv: {}", condition.text),
        }
    }
}
