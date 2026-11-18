//! Conservative presentation shorthand; original assertions remain unchanged.
use {
    super::{
        Diagram,
        property_declaration::{
            Association,
            Attribute,
            Declaration,
            Drawing,
            RangeOnly,
        },
        restriction::{Notation, Restriction, Scope},
    },
    std::collections::BTreeSet,
};
pub(super) const GENERIC_RESOURCE: &str =
    "http://www.w3.org/2000/01/rdf-schema#Resource";

pub(super) struct Shorthand<'a> {
    pub range_only:   Vec<RangeOnly<'a>>,
    pub attributes:   Vec<Attribute<'a>>,
    pub associations: Vec<Association<'a>>,
    /// Restrictions on the documented class.
    pub restrictions: Vec<(Restriction<'a>, Notation<'a>)>,
    /// Restrictions on neighbouring classes, drawn as their members.
    pub neighbours:   Vec<(Restriction<'a>, Notation<'a>)>,
    pub existences:   Vec<super::nested_existence::Existence<'a>>,
    pub consumed:     BTreeSet<usize>,
    pub visible:      BTreeSet<&'a str>,
}
impl<'a> Shorthand<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let mut range_only = Vec::new(); // bounded: eighteen retained facts.
        let mut attributes = Vec::new(); // bounded: eighteen retained facts.
        let mut associations = Vec::new(); // bounded: 81 pairs of eighteen facts.
        let mut consumed = BTreeSet::new(); // bounded: eighteen edge indices.
        let mut visited = BTreeSet::new(); // bounded: thirty-seven nodes.
        for node in diagram.nodes() {
            if !visited.insert(node.id.as_str()) {
                continue;
            }
            let Some(declaration) = Declaration::read(diagram, node) else {
                continue;
            };
            let Some(drawing) = declaration.drawing() else {
                continue;
            };
            consumed.extend(declaration.incident.iter().copied());
            consumed.extend(declaration.union_members(diagram));
            match drawing {
                Drawing::Placeholder => {
                    range_only.extend(
                        declaration
                            .ranges
                            .iter()
                            .map(|range| RangeOnly { property: node, range }),
                    )
                },
                Drawing::Attributes => {
                    attributes.extend(declaration.domains.iter().map(
                        |domain| {
                            Attribute {
                                node,
                                domain,
                                types: declaration.ranges.clone(),
                            }
                        },
                    ));
                },
                Drawing::Associations => {
                    for domain in &declaration.domains {
                        associations.extend(
                            declaration.ranges.iter().map(|range| {
                                Association { node, domain, range }
                            }),
                        );
                    }
                },
            }
        }
        let existences = super::nested_existence::collect(diagram);
        for existence in &existences {
            consumed.extend(&existence.consumed);
        }
        let restrictions =
            Self::notations(diagram, &diagram.edges, Scope::Focus);
        for (restriction, _) in &restrictions {
            consumed.extend(&restriction.consumed);
        }
        let neighbours = Self::notations(
            diagram,
            &diagram.context.restrictions,
            Scope::Neighbour,
        );
        // Suppress only the generic RDF root in this presentation, never facts.
        consumed.extend(
            diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.from.id == GENERIC_RESOURCE ||
                        edge.to.id == GENERIC_RESOURCE
                })
                .map(|(index, _)| index),
        );
        let mut visible = BTreeSet::from([diagram.focus.id.as_str()]); // bounded: thirty-seven nodes.
        for (index, edge) in diagram.edges.iter().enumerate() {
            if !consumed.contains(&index) {
                visible.extend([edge.from.id.as_str(), edge.to.id.as_str()]);
            }
        }
        visible.extend(attributes.iter().map(|a| a.domain.id.as_str()));
        for association in &associations {
            visible.extend([
                association.domain.id.as_str(),
                association.range.id.as_str(),
            ]);
        }
        for (restriction, notation) in &restrictions {
            visible.insert(restriction.class.id.as_str());
            visible.extend(notation.target().map(|t| t.id.as_str()));
        }
        visible.extend(existences.iter().map(|e| e.class.id.as_str()));
        visible.extend(range_only.iter().map(|p| p.range.id.as_str()));
        visible.remove(GENERIC_RESOURCE);
        Self {
            range_only,
            attributes,
            associations,
            restrictions,
            neighbours,
            existences,
            consumed,
            visible,
        }
    }

    fn notations(
        diagram: &'a Diagram,
        edges: &'a [super::Edge],
        scope: Scope,
    ) -> Vec<(Restriction<'a>, Notation<'a>)> {
        Restriction::collect(edges)
            .into_iter()
            .map(|restriction| {
                let notation = restriction.notation(&diagram.context, scope);
                (restriction, notation)
            })
            .collect() // bounded: one per retained fact.
    }

    /// Whether a restriction drawing replaces the plain domain/range drawing
    /// of a property on this class: a member or prohibition, or an
    /// association to the same target.
    pub fn supersedes(
        &self,
        class: &str,
        property: &str,
        target: &str,
    ) -> bool {
        self.restrictions
            .iter()
            .chain(&self.neighbours)
            .any(|(r, n)| {
                r.class.id == class &&
                    r.property.id == property &&
                    match n {
                        Notation::Hidden => false,
                        Notation::Association { target: t, .. } => {
                            t.id == target
                        },
                        Notation::Member { .. } |
                        Notation::Prohibition { .. } => true,
                    }
            })
    }
}
