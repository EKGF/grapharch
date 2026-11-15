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
    },
    std::collections::BTreeSet,
};
pub(super) const GENERIC_RESOURCE: &str =
    "http://www.w3.org/2000/01/rdf-schema#Resource";

pub(super) struct Shorthand<'a> {
    pub range_only:       Vec<RangeOnly<'a>>,
    pub attributes:       Vec<Attribute<'a>>,
    pub associations:     Vec<Association<'a>>,
    pub existentials:     Vec<super::existential::Existential<'a>>,
    pub qualified_counts: Vec<super::qualified_count::QualifiedCount<'a>>,
    pub existences:       Vec<super::nested_existence::Existence<'a>>,
    pub consumed:         BTreeSet<usize>,
    pub visible:          BTreeSet<&'a str>,
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
        let qualified_counts = super::qualified_count::collect(diagram);
        for minimum in &qualified_counts {
            consumed.extend(&minimum.consumed);
        }
        let existentials = super::existential::collect(diagram);
        for existential in &existentials {
            consumed.extend(&existential.consumed);
        }
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
        for existential in &existentials {
            visible.extend([
                existential.class.id.as_str(),
                existential.filler.id.as_str(),
            ]);
        }
        for minimum in &qualified_counts {
            if minimum.visible() {
                visible.insert(minimum.class.id.as_str());
                if !minimum.datatype {
                    visible.insert(minimum.filler.id.as_str());
                }
            }
        }
        visible.extend(existences.iter().map(|e| e.class.id.as_str()));
        visible.extend(range_only.iter().map(|p| p.range.id.as_str()));
        visible.remove(GENERIC_RESOURCE);
        Self {
            range_only,
            attributes,
            associations,
            existentials,
            qualified_counts,
            existences,
            consumed,
            visible,
        }
    }
}
