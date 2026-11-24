//! Bounded reading of one anonymous class expression into its typed form.
//!
//! Every anonymous node must use exactly one recognised OWL constructor.
//! Unknown predicates, repeated constructors, malformed counts, lists or
//! inverse properties make the expression [`Reading::Malformed`]; nesting
//! or size beyond the diagram budget makes it [`Reading::OverBudget`].
use {
    super::{
        model::{
            ClassExpression,
            PropertyExpression,
            Quantifier,
            RestrictionExpression,
        },
        vocabulary::{Annotation, Constructor},
    },
    crate::publication::document::{
        anonymous_order::AnonymousOrder,
        class_diagram::{Node, sources::Sources},
    },
    anyhow::Result,
    futures::future::BoxFuture,
    oxrdf::{NamedNode, Term},
    std::collections::BTreeSet,
};
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
/// The deepest anonymous nesting drawn.
const DEPTH: usize = 4;
/// Named and anonymous resources read for one expression.
const RESOURCES: usize = 24;
/// Members of one list.
pub(in super::super) const MEMBERS: usize = 8;

/// Why an expression has no typed form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Unreadable {
    Malformed,
    OverBudget,
}
type Reading<T> = Result<Result<T, Unreadable>>;

/// Return the unreadable reason from the enclosing reader.
macro_rules! read {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(reason) => return Ok(Err(reason)),
        }
    };
}

/// Reads one expression tree within its resource budget.
pub(in super::super) struct ExpressionReader<'a> {
    sources:   &'a Sources<'a>,
    order:     AnonymousOrder<'a>,
    resources: usize,
}
/// The constructor facts of one anonymous node.
#[derive(Default)]
struct Facts {
    property:   Option<Term>,
    qualifier:  Option<Term>,
    constraint: Option<(Constructor, Term)>,
    construct:  Option<(Constructor, Term)>,
}
impl<'a> ExpressionReader<'a> {
    pub fn new(sources: &'a Sources<'a>) -> Self {
        Self { sources, order: sources.order(), resources: 0 }
    }

    pub async fn read(&mut self, root: &NamedNode) -> Reading<ClassExpression> {
        self.resources = 0;
        self.expression(Term::NamedNode(root.clone()), 0).await
    }

    fn spend(&mut self) -> Result<(), Unreadable> {
        self.resources += 1;
        if self.resources > RESOURCES {
            return Err(Unreadable::OverBudget);
        }
        Ok(())
    }

    fn expression(
        &mut self,
        term: Term,
        depth: usize,
    ) -> BoxFuture<'_, Reading<ClassExpression>> {
        Box::pin(async move {
            read!(self.spend());
            let Term::NamedNode(node) = &term else {
                return Ok(Err(Unreadable::Malformed));
            };
            if !AnonymousOrder::is_anonymous(&term) {
                return Ok(Ok(ClassExpression::Named(
                    self.sources.node(node).await?,
                )));
            }
            if depth == DEPTH {
                return Ok(Err(Unreadable::OverBudget));
            }
            let facts = read!(self.facts(node).await?);
            if let Some((constraint, value)) = facts.constraint {
                if facts.construct.is_some() {
                    return Ok(Err(Unreadable::Malformed));
                }
                let Some(property) = facts.property else {
                    return Ok(Err(Unreadable::Malformed));
                };
                let property = read!(self.property(property).await?);
                let quantifier = read!(
                    self.quantifier(constraint, value, facts.qualifier, depth)
                        .await?
                );
                return Ok(Ok(ClassExpression::Restriction(Box::new(
                    RestrictionExpression { property, quantifier },
                ))));
            }
            let (Some((construct, value)), None, None) =
                (facts.construct, facts.property, facts.qualifier)
            else {
                return Ok(Err(Unreadable::Malformed));
            };
            Ok(Ok(match construct {
                Constructor::UnionOf | Constructor::IntersectionOf => {
                    let mut parts = Vec::new(); // bounded: MEMBERS.
                    for member in read!(self.list(value).await?) {
                        parts.push(read!(
                            self.expression(member, depth + 1).await?
                        ));
                    }
                    if parts.len() < 2 {
                        return Ok(Err(Unreadable::Malformed));
                    }
                    if construct == Constructor::UnionOf {
                        ClassExpression::Union(parts)
                    } else {
                        ClassExpression::Intersection(parts)
                    }
                },
                Constructor::ComplementOf |
                Constructor::DatatypeComplementOf => {
                    ClassExpression::Complement(Box::new(read!(
                        self.expression(value, depth + 1).await?
                    )))
                },
                Constructor::OneOf => {
                    let mut values = Vec::new(); // bounded: MEMBERS.
                    for member in read!(self.list(value).await?) {
                        values.push(read!(self.value(member).await?));
                    }
                    if values.is_empty() {
                        return Ok(Err(Unreadable::Malformed));
                    }
                    ClassExpression::OneOf(values)
                },
                _ => return Ok(Err(Unreadable::Malformed)),
            }))
        })
    }

    /// Sort the facts of an anonymous node by role, rejecting anything else.
    async fn facts(&self, node: &NamedNode) -> Reading<Facts> {
        let mut facts = Facts::default();
        for (predicate, value) in self.order.facts(node).await? {
            if predicate.as_str() == format!("{RDF}type") {
                let known = matches!(&value, Term::NamedNode(kind) if Annotation::is_expression_type(kind.as_str()));
                if !known {
                    return Ok(Err(Unreadable::Malformed));
                }
                continue;
            }
            if Annotation::is_annotation(predicate.as_str()) {
                continue;
            }
            let Some(constructor) = Constructor::parse(predicate.as_str())
            else {
                return Ok(Err(Unreadable::Malformed));
            };
            let slot = match constructor {
                Constructor::OnProperty => {
                    if facts.property.replace(value).is_some() {
                        return Ok(Err(Unreadable::Malformed));
                    }
                    continue;
                },
                Constructor::OnClass | Constructor::OnDataRange => {
                    if facts.qualifier.replace(value).is_some() {
                        return Ok(Err(Unreadable::Malformed));
                    }
                    continue;
                },
                _ if constructor.is_constraint() => &mut facts.constraint,
                _ => &mut facts.construct,
            };
            if slot.replace((constructor, value)).is_some() {
                return Ok(Err(Unreadable::Malformed));
            }
        }
        Ok(Ok(facts))
    }

    async fn property(&mut self, term: Term) -> Reading<PropertyExpression> {
        read!(self.spend());
        let Term::NamedNode(node) = &term else {
            return Ok(Err(Unreadable::Malformed));
        };
        if !AnonymousOrder::is_anonymous(&term) {
            return Ok(Ok(PropertyExpression::Named(
                self.sources.node(node).await?,
            )));
        }
        let mut inverse = None;
        for (predicate, value) in self.order.facts(node).await? {
            match (Constructor::parse(predicate.as_str()), &value) {
                (Some(Constructor::InverseOf), Term::NamedNode(named))
                    if inverse.is_none() &&
                        !AnonymousOrder::is_anonymous(&value) =>
                {
                    inverse = Some(named.clone());
                },
                _ if predicate.as_str() == format!("{RDF}type") ||
                    Annotation::is_annotation(predicate.as_str()) => {},
                _ => return Ok(Err(Unreadable::Malformed)),
            }
        }
        let Some(inverse) = inverse else {
            return Ok(Err(Unreadable::Malformed));
        };
        Ok(Ok(PropertyExpression::Inverse(
            self.sources.node(&inverse).await?,
        )))
    }

    async fn quantifier(
        &mut self,
        constraint: Constructor,
        value: Term,
        qualifier: Option<Term>,
        depth: usize,
    ) -> Reading<Quantifier> {
        if let Some((bound, qualified)) = constraint.bound() {
            if qualified != qualifier.is_some() {
                return Ok(Err(Unreadable::Malformed));
            }
            let Term::Literal(literal) = &value else {
                return Ok(Err(Unreadable::Malformed));
            };
            let Some(count) = Node::literal(literal).count() else {
                return Ok(Err(Unreadable::Malformed));
            };
            let filler = match qualifier {
                Some(filler) => {
                    Some(read!(self.expression(filler, depth + 1).await?))
                },
                None => None,
            };
            return Ok(Ok(Quantifier::Count { bound, count, filler }));
        }
        if qualifier.is_some() {
            return Ok(Err(Unreadable::Malformed));
        }
        Ok(Ok(match constraint {
            Constructor::SomeValuesFrom => {
                Quantifier::Some(read!(
                    self.expression(value, depth + 1).await?
                ))
            },
            Constructor::AllValuesFrom => {
                Quantifier::All(read!(self.expression(value, depth + 1).await?))
            },
            Constructor::HasValue => {
                Quantifier::Value(read!(self.value(value).await?))
            },
            Constructor::HasSelf => {
                let truth =
                    matches!(&value, Term::Literal(l) if l.value() == "true");
                if !truth {
                    return Ok(Err(Unreadable::Malformed));
                }
                Quantifier::Myself
            },
            _ => return Ok(Err(Unreadable::Malformed)),
        }))
    }

    /// A named individual or a data value.
    async fn value(&mut self, term: Term) -> Reading<Node> {
        read!(self.spend());
        Ok(Ok(match &term {
            Term::Literal(literal) => Node::literal(literal),
            Term::NamedNode(node) if !AnonymousOrder::is_anonymous(&term) => {
                self.sources.node(node).await?
            },
            _ => return Ok(Err(Unreadable::Malformed)),
        }))
    }

    /// The members of one well-formed, acyclic RDF list.
    async fn list(&mut self, head: Term) -> Reading<Vec<Term>> {
        let mut members = Vec::new(); // bounded: MEMBERS.
        let mut visited = BTreeSet::new(); // bounded: MEMBERS cells.
        let mut cell = head;
        loop {
            let Term::NamedNode(node) = &cell else {
                return Ok(Err(Unreadable::Malformed));
            };
            if node.as_str() == format!("{RDF}nil") {
                return Ok(Ok(members));
            }
            if members.len() == MEMBERS {
                return Ok(Err(Unreadable::OverBudget));
            }
            if !AnonymousOrder::is_anonymous(&cell) ||
                !visited.insert(node.clone())
            {
                return Ok(Err(Unreadable::Malformed));
            }
            let (mut first, mut rest) = (None, None);
            for (predicate, value) in self.order.facts(node).await? {
                let slot = match predicate.as_str().strip_prefix(RDF) {
                    Some("first") => &mut first,
                    Some("rest") => &mut rest,
                    Some("type") => continue,
                    _ => return Ok(Err(Unreadable::Malformed)),
                };
                if slot.replace(value).is_some() {
                    return Ok(Err(Unreadable::Malformed));
                }
            }
            let (Some(first), Some(rest)) = (first, rest) else {
                return Ok(Err(Unreadable::Malformed));
            };
            members.push(first);
            cell = rest;
        }
    }
}
