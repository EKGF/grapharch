//! Typed anonymous OWL class expressions, read bounded from authored syntax.
use {
    super::super::Node,
    serde::{Deserialize, Serialize},
};

/// An OWL 2 class or data range expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum ClassExpression {
    /// A named class or datatype.
    Named(Node),
    Restriction(Box<RestrictionExpression>),
    Union(Vec<ClassExpression>), // bounded: MEMBERS.
    Intersection(Vec<ClassExpression>), // bounded: MEMBERS.
    Complement(Box<ClassExpression>),
    /// An enumeration of individuals or data values.
    OneOf(Vec<Node>), // bounded: MEMBERS.
}
/// A property or the inverse of a named property.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum PropertyExpression {
    Named(Node),
    Inverse(Node),
}
/// Which bound a cardinality states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Bound {
    Minimum,
    Exact,
    Maximum,
}
/// What a restriction requires of its property's values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum Quantifier {
    Some(ClassExpression),
    All(ClassExpression),
    Value(Node),
    /// `owl:hasSelf`: the individual is among its own values.
    Myself,
    Count {
        bound:  Bound,
        count:  u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filler: Option<ClassExpression>,
    },
}
/// One OWL property restriction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RestrictionExpression {
    pub property:   PropertyExpression,
    pub quantifier: Quantifier,
}
/// How a class relates to an anonymous expression.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AxiomRelation {
    /// `C rdfs:subClassOf E`.
    Subclass,
    /// `C owl:equivalentClass E`, in either direction.
    Equivalent,
    /// `C owl:disjointWith E`, in either direction.
    Disjoint,
    /// `E rdfs:subClassOf C`.
    Superclass,
}
/// One asserted axiom relating a named class to an anonymous expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClassAxiom {
    pub class:      Node,
    pub relation:   AxiomRelation,
    /// The stable identity of the anonymous expression.
    pub id:         String,
    pub expression: ClassExpression,
}
const THING: &str = "http://www.w3.org/2002/07/owl#Thing";

impl ClassExpression {
    /// Every named resource the expression mentions, in syntax order.
    pub fn nodes(&self) -> Vec<&Node> {
        let mut nodes = Vec::new(); // bounded: the expression's node budget.
        self.visit(&mut |node| nodes.push(node));
        nodes
    }

    pub fn nodes_mut(&mut self) -> Vec<&mut Node> {
        let mut nodes = Vec::new(); // bounded: the expression's node budget.
        self.visit_mut(&mut nodes);
        nodes
    }

    fn visit<'a>(&'a self, f: &mut impl FnMut(&'a Node)) {
        match self {
            Self::Named(node) => f(node),
            Self::Restriction(restriction) => restriction.visit(f),
            Self::Union(parts) | Self::Intersection(parts) => {
                parts.iter().for_each(|part| part.visit(f));
            },
            Self::Complement(inner) => inner.visit(f),
            Self::OneOf(values) => values.iter().for_each(f),
        }
    }

    fn visit_mut<'a>(&'a mut self, nodes: &mut Vec<&'a mut Node>) {
        match self {
            Self::Named(node) => nodes.push(node),
            Self::Restriction(restriction) => restriction.visit_mut(nodes),
            Self::Union(parts) | Self::Intersection(parts) => {
                parts.iter_mut().for_each(|part| part.visit_mut(nodes));
            },
            Self::Complement(inner) => inner.visit_mut(nodes),
            Self::OneOf(values) => nodes.extend(values.iter_mut()),
        }
    }

    /// The named class or datatype, when the expression is one.
    pub fn named(&self) -> Option<&Node> {
        match self {
            Self::Named(node) => Some(node),
            _ => None,
        }
    }

    /// Whether every individual satisfies the expression: `owl:Thing`, or a
    /// restriction requiring at least zero values.
    pub fn is_top(&self) -> bool {
        match self {
            Self::Named(node) => node.id == THING,
            Self::Restriction(restriction) => {
                matches!(restriction.quantifier, Quantifier::Count {
                    bound: Bound::Minimum,
                    count: 0,
                    ..
                })
            },
            _ => false,
        }
    }
}
impl RestrictionExpression {
    fn visit<'a>(&'a self, f: &mut impl FnMut(&'a Node)) {
        f(self.property.node());
        match &self.quantifier {
            Quantifier::Some(filler) | Quantifier::All(filler) => {
                filler.visit(f)
            },
            Quantifier::Value(value) => f(value),
            Quantifier::Myself => {},
            Quantifier::Count { filler, .. } => {
                if let Some(filler) = filler {
                    filler.visit(f);
                }
            },
        }
    }

    fn visit_mut<'a>(&'a mut self, nodes: &mut Vec<&'a mut Node>) {
        nodes.push(self.property.node_mut());
        match &mut self.quantifier {
            Quantifier::Some(filler) | Quantifier::All(filler) => {
                filler.visit_mut(nodes)
            },
            Quantifier::Value(value) => nodes.push(value),
            Quantifier::Myself => {},
            Quantifier::Count { filler, .. } => {
                if let Some(filler) = filler {
                    filler.visit_mut(nodes);
                }
            },
        }
    }
}
impl PropertyExpression {
    /// The named property, also of an inverse.
    pub fn node(&self) -> &Node {
        match self {
            Self::Named(node) | Self::Inverse(node) => node,
        }
    }

    fn node_mut(&mut self) -> &mut Node {
        match self {
            Self::Named(node) | Self::Inverse(node) => node,
        }
    }
}
impl ClassAxiom {
    pub fn nodes(&self) -> Vec<&Node> {
        let mut nodes = vec![&self.class]; // bounded: the expression's node budget.
        nodes.extend(self.expression.nodes());
        nodes
    }

    pub fn nodes_mut(&mut self) -> Vec<&mut Node> {
        let mut nodes = vec![&mut self.class]; // bounded: the expression's node budget.
        nodes.extend(self.expression.nodes_mut());
        nodes
    }
}
