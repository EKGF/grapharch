//! Conservative presentation shorthand; original assertions remain unchanged.
use {
    super::{
        Diagram,
        axiom_plan::AxiomPlan,
        property_declaration::{
            Association,
            Attribute,
            Declaration,
            Drawing,
            RangeOnly,
        },
        restriction::{Constraint, Notation, Restriction, Scope},
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
    /// Generalizations, notes and enumerations of the documented class.
    pub plan:         AxiomPlan<'a>,
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
        let mut plan = AxiomPlan::new(&diagram.axioms, Scope::Focus);
        let restrictions = Self::notations(
            diagram,
            std::mem::take(&mut plan.restrictions),
            Scope::Focus,
        );
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
                        associations.extend(declaration.ranges.iter().map(
                            |range| {
                                Association {
                                    node,
                                    domain,
                                    range,
                                    alternative: declaration.alternatives,
                                }
                            },
                        ));
                    }
                },
            }
        }
        let neighbours = Self::notations(
            diagram,
            AxiomPlan::new(&diagram.context.axioms, Scope::Neighbour)
                .restrictions,
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
        for generalization in &plan.generalizations {
            visible.extend([
                generalization.child.id.as_str(),
                generalization.parent.id.as_str(),
            ]);
        }
        visible.extend(plan.notes.iter().map(|n| n.class.id.as_str()));
        visible.extend(range_only.iter().map(|p| p.range.id.as_str()));
        visible.remove(GENERIC_RESOURCE);
        Self {
            range_only,
            attributes,
            associations,
            restrictions,
            neighbours,
            plan,
            consumed,
            visible,
        }
    }

    fn notations(
        diagram: &'a Diagram,
        restrictions: Vec<Restriction<'a>>,
        scope: Scope,
    ) -> Vec<(Restriction<'a>, Notation<'a>)> {
        let covered: Vec<(&str, &str)> = restrictions
            .iter()
            .filter(|r| r.guarantees_existence())
            .map(|r| (r.class.id.as_str(), r.property.id.as_str()))
            .collect(); // bounded: one per drawn restriction.
        restrictions
            .into_iter()
            .map(|restriction| {
                // A positive count on the same class and property already
                // guarantees the value an untyped existential requires.
                let notation =
                    if matches!(restriction.constraint, Constraint::Exists) &&
                        !restriction.inverse &&
                        covered.contains(&(
                            restriction.class.id.as_str(),
                            restriction.property.id.as_str(),
                        ))
                    {
                        Notation::Hidden
                    } else {
                        restriction.notation(&diagram.context, scope)
                    };
                (restriction, notation)
            })
            .collect() // bounded: one per drawn restriction.
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
            .chain(
                // A neighbour's member never hides the association linking
                // it to the documented class; only attributes are replaced.
                self.neighbours.iter().filter(|_| target.is_empty()),
            )
            .any(|(r, n)| {
                r.class.id == class &&
                    r.property.id == property &&
                    r.path.is_empty() &&
                    match n {
                        Notation::Hidden => false,
                        Notation::Association { target: t, .. } => {
                            t.id == target
                        },
                        Notation::Member { .. } |
                        Notation::Prohibition { .. } => !r.inverse,
                    }
            })
    }
}
