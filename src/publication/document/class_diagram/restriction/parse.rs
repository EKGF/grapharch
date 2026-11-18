//! Recognise complete simple restrictions in a bounded set of retained facts.
//!
//! A restriction is simplified only when its node has exactly one incoming
//! `rdfs:subClassOf` from a named class and every outgoing predicate is part
//! of one recognised form. Extra predicates, shared nodes, inverse or other
//! anonymous properties, complex fillers and malformed counts keep the
//! explicit expression box.
use {
    super::{
        form::{Constraint, Filler, Restriction},
        vocabulary::{Bound, SUBCLASS, Term},
    },
    crate::publication::document::class_diagram::{
        Edge,
        Node,
        NodeKind,
        multiplicity::Cardinality,
        shorthand::GENERIC_RESOURCE,
    },
};

impl<'a> Filler<'a> {
    fn named(node: &'a Node, data: bool) -> Option<Self> {
        (node.kind.is_class_like() && node.id != GENERIC_RESOURCE)
            .then_some(Self { node, data })
    }
}
impl<'a> Restriction<'a> {
    /// Every simple restriction in these facts, in fact order.
    pub fn collect(edges: &'a [Edge]) -> Vec<Self> {
        edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| Self::parse(edges, index, edge))
            .collect() // bounded: one per retained fact.
    }

    fn parse(edges: &'a [Edge], index: usize, edge: &'a Edge) -> Option<Self> {
        if edge.predicate.as_deref() != Some(SUBCLASS) ||
            edge.from.kind != NodeKind::Class ||
            edge.to.kind != NodeKind::OwlExpression
        {
            return None;
        }
        let node = &edge.to.id;
        let mut consumed = vec![index]; // bounded: four facts.
        let mut terms = Vec::<(Term, &Node)>::new(); // bounded: three terms.
        for (other, fact) in edges.iter().enumerate() {
            if other == index || (fact.from.id != *node && fact.to.id != *node)
            {
                continue;
            }
            // A shared node or a further predicate prevents simplification.
            if fact.from.id != *node || terms.len() == 3 {
                return None;
            }
            let term = Term::parse(fact.predicate.as_deref()?)?;
            if terms.iter().any(|(t, _)| *t == term) {
                return None;
            }
            terms.push((term, &fact.to));
            consumed.push(other);
        }
        let get =
            |wanted| terms.iter().find(|(t, _)| *t == wanted).map(|(_, n)| *n);
        let property = get(Term::OnProperty)?;
        if !matches!(
            property.kind,
            NodeKind::ObjectProperty |
                NodeKind::DatatypeProperty |
                NodeKind::RdfProperty |
                NodeKind::Resource
        ) {
            return None;
        }
        let constraint = match terms.len() {
            2 => Self::unqualified(property, &terms)?,
            3 => Self::qualified_count(property, &get)?,
            _ => return None,
        };
        Some(Self { class: &edge.from, property, constraint, consumed })
    }

    fn unqualified(
        property: &'a Node,
        terms: &[(Term, &'a Node)],
    ) -> Option<Constraint<'a>> {
        let (term, value) = terms
            .iter()
            .copied()
            .find(|(t, _)| *t != Term::OnProperty)?;
        match term {
            Term::SomeValuesFrom | Term::AllValuesFrom => {
                let filler = Filler::named(value, value.datatype)?;
                // An undeclared predicate is drawn as an association only
                // when its filler is a declared class.
                let compatible = if filler.data {
                    property.kind != NodeKind::ObjectProperty
                } else {
                    property.kind == NodeKind::ObjectProperty ||
                        (property.kind != NodeKind::DatatypeProperty &&
                            value.kind == NodeKind::Class)
                };
                compatible.then_some(
                    if term == Term::SomeValuesFrom {
                        Constraint::Some(filler)
                    } else {
                        Constraint::All(filler)
                    },
                )
            },
            Term::HasValue => {
                let compatible = if value.kind == NodeKind::LiteralValue {
                    property.kind != NodeKind::ObjectProperty
                } else {
                    property.kind != NodeKind::DatatypeProperty &&
                        value.kind.is_class_like()
                };
                compatible.then_some(Constraint::Value(value))
            },
            _ => {
                let (bound, false) = term.bound()? else {
                    return None;
                };
                Some(Constraint::Count {
                    cardinality: bound.cardinality(value.count()?),
                    filler:      None,
                })
            },
        }
    }

    fn qualified_count(
        property: &'a Node,
        get: &impl Fn(Term) -> Option<&'a Node>,
    ) -> Option<Constraint<'a>> {
        let filler = match (get(Term::OnClass), get(Term::OnDataRange)) {
            (Some(class), None) => Filler::named(class, false)?,
            (None, Some(range)) => Filler::named(range, true)?,
            _ => return None,
        };
        let compatible = if filler.data {
            property.kind != NodeKind::ObjectProperty
        } else {
            property.kind != NodeKind::DatatypeProperty
        };
        if !compatible {
            return None;
        }
        let (bound, count) = [
            Term::MinQualifiedCardinality,
            Term::QualifiedCardinality,
            Term::MaxQualifiedCardinality,
        ]
        .into_iter()
        .find_map(|term| Some((term.bound()?.0, get(term)?)))?;
        Some(Constraint::Count {
            cardinality: bound.cardinality(count.count()?),
            filler:      Some(filler),
        })
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
