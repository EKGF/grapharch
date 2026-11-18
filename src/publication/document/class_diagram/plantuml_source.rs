//! Deterministic PlantUML source for one retained class diagram.
use {
    super::{
        Diagram,
        Node,
        NodeKind,
        constraint_projection,
        escape::Escaped,
        layout::{self, Layout},
        shorthand::Shorthand,
    },
    std::collections::{BTreeMap, BTreeSet},
};

const HEADER: &str =
    "@startuml\n!pragma layout smetana\nallowmixing\nleft to right \
     direction\nhide empty members\nhide circle\nskinparam shadowing \
     false\nskinparam backgroundColor transparent\nskinparam defaultFontName \
     Arial\nskinparam defaultFontSize 14\nskinparam ArrowColor \
     #68758A\nskinparam nodesep 30\nskinparam ranksep 45\n";

pub(super) struct PlantUmlSource<'a> {
    diagram:     &'a Diagram,
    nodes:       BTreeMap<&'a str, &'a Node>, // bounded: thirty-seven nodes.
    identifiers: BTreeMap<&'a str, usize>,    // bounded: thirty-seven nodes.
    shorthand:   Shorthand<'a>,
    layout:      Layout<'a>,
    visible:     BTreeSet<&'a str>, // bounded: thirty-seven nodes.
    output:      String,
}
impl<'a> PlantUmlSource<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let nodes = Self::index(diagram);
        let identifiers = nodes
            .keys()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect(); // bounded: thirty-seven nodes.
        let shorthand = Shorthand::new(diagram);
        let mut visible = shorthand.visible.clone();
        for (class, field) in constraint_projection::properties(diagram) {
            visible.insert(class.id.as_str());
            if field.object &&
                !field.prohibited() &&
                let Some(range) = field.range
            {
                visible.insert(range.id.as_str());
            }
        }
        Self {
            diagram,
            nodes,
            identifiers,
            shorthand,
            layout: Layout::new(diagram),
            visible,
            output: String::new(),
        }
    }

    /// Every distinct node in deterministic identity order.
    pub fn index(diagram: &Diagram) -> BTreeMap<&str, &Node> {
        diagram
            .nodes()
            .map(|node| (node.id.as_str(), node))
            .collect() // bounded: at most thirty-seven nodes.
    }

    pub fn render(mut self) -> String {
        self.output.push_str(
            &if self.layout.has_hierarchy() {
                HEADER.replacen(
                    "left to right direction",
                    "top to bottom direction",
                    1,
                )
            } else {
                HEADER.to_owned()
            },
        );
        let nodes: Vec<_> = self.nodes.values().copied().collect(); // bounded: thirty-seven nodes.
        for node in nodes {
            if self.visible.contains(node.id.as_str()) {
                self.node(node);
            }
        }
        self.placeholder();
        self.associations();
        self.restrictions();
        self.remaining_facts();
        self.output.push_str("@enduml\n");
        self.output
    }

    fn id(&self, node: &Node) -> usize { self.identifiers[node.id.as_str()] }

    fn node(&mut self, node: &Node) {
        let index = self.id(node);
        let color = if node.id == self.diagram.focus.id {
            "#E9EDFF"
        } else if node.kind.is_property() {
            "#FFF5E9"
        } else {
            "#FFFFFF"
        };
        // Opaque links resolve after layout; source URLs never enter PlantUML.
        let link = format!("[[./grapharch-node/n{index}]]");
        let owns_attributes = self
            .shorthand
            .attributes
            .iter()
            .any(|a| a.domain.id == node.id);
        if node.kind == NodeKind::Class || owns_attributes {
            let stereotype = if node.kind == NodeKind::Class {
                "owl:Class".to_owned()
            } else {
                Escaped::inline(&node.kind.to_string()).to_string()
            };
            self.output.push_str(&format!(
                "class \"{}\" as n{index} <<{stereotype}>> {link} {color} {{\n",
                Escaped::wrapped(&node.label)
            ));
            self.members(node);
            self.output.push_str("}\n");
        } else if node.kind == NodeKind::OwlUnion {
            self.output.push_str(&format!(
                "rectangle \"{}\" as n{index} {link} {color}\n",
                self.union_label(node)
            ));
        } else {
            self.output.push_str(&format!(
                "rectangle \"<<{}>>\\n{}\" as n{index} {link} {color}\n",
                Escaped::inline(&node.kind.to_string()),
                Escaped::wrapped(&node.label)
            ));
        }
    }

    fn union_label(&self, node: &Node) -> Escaped {
        let mut members = self
            .diagram
            .edges
            .iter()
            .filter(|edge| edge.from.id == node.id && edge.declares_union())
            .map(|edge| edge.to.label.as_str())
            .collect::<Vec<_>>(); // bounded: eight complete union members.
        members.sort_unstable();
        Escaped::wrapped(&members.join(" or "))
    }

    fn members(&mut self, node: &Node) {
        let mut fields = String::new();
        for attribute in self.shorthand.attributes.iter().filter(|a| {
            a.domain.id == node.id &&
                !self.shorthand.supersedes(&node.id, &a.node.id, "") &&
                !constraint_projection::constrained(
                    self.diagram,
                    &node.id,
                    &a.node.id,
                )
        }) {
            let types = attribute
                .types
                .iter()
                .map(|t| t.label.as_str())
                .collect::<Vec<_>>() // bounded: eighteen retained facts.
                .join(" and ");
            let types = if types.is_empty() {
                String::new()
            } else {
                format!(" : {}", Escaped::inline(&types))
            };
            fields.push_str(&format!(
                "  {{field}} {}{types} [[[./grapharch-node/n{}]]]\n",
                Escaped::inline(&attribute.node.label),
                self.id(attribute.node)
            ));
        }
        for existence in self
            .shorthand
            .existences
            .iter()
            .filter(|e| e.class.id == node.id)
        {
            existence.write(
                &mut fields,
                &self.identifiers,
                &self.shorthand.restrictions,
            );
        }
        let mut constraints = String::new();
        for (_, notation) in self
            .shorthand
            .restrictions
            .iter()
            .chain(&self.shorthand.neighbours)
            .filter(|(r, _)| r.class.id == node.id)
        {
            fields.extend(notation.member(&self.identifiers));
            constraints.extend(notation.constraint(&self.identifiers));
        }
        constraint_projection::members(
            &mut fields,
            &mut constraints,
            node,
            self.diagram,
            &self.identifiers,
        );
        // Constraint rows follow the attributes. A PlantUML separator would
        // be emitted inside the preceding member's link and break hit regions.
        self.output.push_str(&fields);
        self.output.push_str(&constraints);
    }

    fn placeholder(&mut self) {
        if self.shorthand.range_only.is_empty() {
            return;
        }
        self.output.push_str(
            "class \"rdfs:Resource\" as unknownDomain \
             #white;line:gray;line.dotted\n",
        );
        let mut lines = String::new();
        for property in &self.shorthand.range_only {
            lines.push_str(&format!(
                "unknownDomain ..> n{} [[./grapharch-node/n{}]] : {}\n",
                self.id(property.range),
                self.id(property.property),
                Escaped::wrapped(&property.property.label)
            ));
        }
        self.output.push_str(&lines);
    }

    fn associations(&mut self) {
        let mut lines = String::new();
        for association in self.shorthand.associations.iter().filter(|a| {
            !self
                .shorthand
                .supersedes(&a.domain.id, &a.node.id, &a.range.id)
        }) {
            lines.push_str(&format!(
                "n{} {} {}n{} [[./grapharch-node/n{}]] : {}\n",
                self.id(association.domain),
                self.layout.association_arrow(
                    &association.domain.id,
                    &association.range.id
                ),
                constraint_projection::association_count(
                    self.diagram,
                    &association.domain.id,
                    &association.node.id,
                    &association.range.id
                ),
                self.id(association.range),
                self.id(association.node),
                Escaped::wrapped(&association.node.label)
            ));
        }
        self.output.push_str(&lines);
        let associations = &self.shorthand.associations;
        constraint_projection::associations(
            &mut self.output,
            self.diagram,
            &self.identifiers,
            |class, field| {
                associations.iter().any(|a| {
                    a.domain.id == class &&
                        a.node.id == field.property.id &&
                        field.range.is_some_and(|r| r.id == a.range.id)
                })
            },
        );
    }

    fn restrictions(&mut self) {
        let mut lines = String::new();
        for (restriction, notation) in &self.shorthand.restrictions {
            lines.extend(notation.association(
                restriction.class,
                &self.identifiers,
                &self.layout,
            ));
        }
        self.output.push_str(&lines);
    }

    fn remaining_facts(&mut self) {
        let mut lines = String::new();
        for (index, edge) in self.diagram.edges.iter().enumerate() {
            if self.shorthand.consumed.contains(&index) {
                continue;
            }
            let generalization = layout::is_generalization(edge);
            lines.push_str(&format!(
                "n{} {} n{}{}\n",
                self.id(&edge.from),
                if generalization {
                    self.layout.generalization_arrow(edge)
                } else {
                    self.layout.relation_arrow(edge)
                },
                self.id(&edge.to),
                if generalization {
                    String::new()
                } else {
                    format!(" : {}", Escaped::wrapped(&edge.relation))
                }
            ));
        }
        self.output.push_str(&lines);
    }
}
