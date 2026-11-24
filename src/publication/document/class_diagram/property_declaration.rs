//! How one RDF property's domain and range declarations are drawn.
//!
//! A property is a labelled edge, never a box with `rdfs:domain` and
//! `rdfs:range` arrows, whenever its declarations allow it:
//!
//! - Object, annotation and generic RDF properties draw one association from
//!   every declared domain to every declared range. Multiple declarations are
//!   conjunctive, so each drawn pair holds for every use of the property.
//! - Datatype properties, and properties without a specific range, are
//!   attributes of every declared domain, typed with all declared ranges. A
//!   domain or range that is an anonymous OWL expression cannot own or type a
//!   member, so such declarations are associations to the expression instead.
//! - Without a specific domain, an association starts at the dotted
//!   `rdfs:Resource` presentation placeholder. An attribute needs an owner, so
//!   a property with neither a specific domain nor a specific range, or a
//!   datatype property without a domain, stays an explicit resource.
//! - A property that takes part in any other drawn relationship, such as an
//!   explicit OWL expression, stays explicit so that relationship keeps its
//!   endpoint.
//! - A single explicit union domain is drawn as its member classes: one
//!   association per member, tagged `{xor}`, or the attribute on each member.
use {
    super::{Diagram, Node, NodeKind, shorthand::GENERIC_RESOURCE},
    std::collections::BTreeSet,
};

/// A property drawn as a member of its domain.
pub(super) struct Attribute<'a> {
    pub node:   &'a Node,
    pub domain: &'a Node,
    pub types:  Vec<&'a Node>, // bounded: eighteen retained facts.
}
/// A property drawn as a labelled edge from one domain to one range.
pub(super) struct Association<'a> {
    pub node:        &'a Node,
    pub domain:      &'a Node,
    pub range:       &'a Node,
    /// The domain is one member of a union domain: an exclusive end.
    pub alternative: bool,
}
/// A property drawn as an edge from the domain placeholder to one range.
pub(super) struct RangeOnly<'a> {
    pub property: &'a Node,
    pub range:    &'a Node,
}

pub(super) enum Drawing {
    Attributes,
    Associations,
    Placeholder,
}

/// The complete, unshared domain and range declarations of one property.
pub(super) struct Declaration<'a> {
    node:             &'a Node,
    pub domains:      Vec<&'a Node>, // bounded: eighteen retained facts.
    pub ranges:       Vec<&'a Node>, // bounded: eighteen retained facts.
    pub incident:     BTreeSet<usize>, // bounded: eighteen edge indices.
    /// The domains are the members of one explicit union domain.
    pub alternatives: bool,
    unions:           Vec<&'a str>, // bounded: one union domain.
}
impl<'a> Declaration<'a> {
    pub fn read(diagram: &'a Diagram, node: &'a Node) -> Option<Self> {
        if !(node.kind.is_property() || node.kind == NodeKind::Resource) {
            return None;
        }
        let mut declaration = Self {
            node,
            domains: Vec::new(),
            ranges: Vec::new(),
            incident: BTreeSet::new(),
            alternatives: false,
            unions: Vec::new(),
        };
        for (index, edge) in diagram.edges.iter().enumerate() {
            if edge.from.id != node.id && edge.to.id != node.id {
                continue;
            }
            if edge.from.id != node.id ||
                edge.to.kind == NodeKind::LiteralValue ||
                !(edge.declares_domain() || edge.declares_range())
            {
                return None;
            }
            declaration.incident.insert(index);
            // The generic RDF root is never a specific domain or range.
            if edge.to.id == GENERIC_RESOURCE {
                continue;
            }
            if edge.declares_domain() {
                declaration.domains.push(&edge.to);
            } else {
                declaration.ranges.push(&edge.to);
            }
        }
        if let [union] = declaration.domains.as_slice() &&
            union.kind == NodeKind::OwlUnion
        {
            declaration.unions.push(union.id.as_str());
            declaration.domains = diagram
                .edges
                .iter()
                .filter(|e| e.from.id == union.id && e.declares_union())
                .map(|e| &e.to)
                .collect(); // bounded: eight union members.
            declaration.alternatives = true;
        }
        (!declaration.incident.is_empty()).then_some(declaration)
    }

    pub fn drawing(&self) -> Option<Drawing> {
        let attribute = self.node.kind == NodeKind::DatatypeProperty ||
            self.ranges.is_empty();
        if self.domains.is_empty() {
            return (!attribute).then_some(Drawing::Placeholder);
        }
        if !attribute {
            return Some(Drawing::Associations);
        }
        if self.domains.iter().all(|d| d.kind.is_class_like()) &&
            self.ranges.iter().all(|r| !r.kind.is_expression())
        {
            Some(Drawing::Attributes)
        } else {
            (!self.ranges.is_empty()).then_some(Drawing::Associations)
        }
    }

    /// Membership facts of the union domain its members replace.
    pub fn union_members(
        &self,
        diagram: &'a Diagram,
    ) -> impl Iterator<Item = usize> + 'a {
        let unions = self.unions.clone();
        diagram
            .edges
            .iter()
            .enumerate()
            .filter(move |(_, edge)| {
                unions.contains(&edge.from.id.as_str()) && edge.declares_union()
            })
            .map(|(index, _)| index)
    }
}
