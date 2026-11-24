//! Class boxes, their stereotypes and compartments.
use {
    super::source::PlantUmlSource,
    crate::publication::document::class_diagram::{
        Node,
        NodeKind,
        constraint_projection,
        escape::Escaped,
        verbalization::Qualifiers,
    },
};

impl<'a> PlantUmlSource<'a> {
    pub(super) fn node(&mut self, node: &'a Node) {
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
        if node.kind.is_class_like() || owns_attributes {
            let stereotype = self.stereotype(node);
            self.words.class(node, stereotype);
            let stereotype =
                stereotype.map(|s| format!(" <<{s}>>")).unwrap_or_default();
            self.output.push_str(&format!(
                "class \"{}\" as n{index}{stereotype} {link} {color} {{\n",
                Self::name(node)
            ));
            self.members(node);
            self.output.push_str("}\n");
        } else {
            self.words.element(node);
            self.output.push_str(&format!(
                "rectangle \"<<{}>>\\n{}\" as n{index} {link} {color}\n",
                Escaped::inline(&node.kind.to_string()),
                Escaped::wrapped(&node.label)
            ));
        }
    }

    /// The label, and the vocabulary under a name another box shares.
    fn name(node: &Node) -> String {
        match &node.from {
            Some(from) => {
                format!(
                    "{}\\n{}",
                    Escaped::wrapped(&node.label),
                    Escaped::inline(&format!("(from {from})"))
                )
            },
            None => Escaped::wrapped(&node.label).to_string(),
        }
    }

    /// Only discriminating stereotypes: a declared OWL class has none.
    fn stereotype(&self, node: &Node) -> Option<&'static str> {
        if self.enumeration(node).is_some() {
            Some("enumeration")
        } else if node.metaclass().is_some() {
            Some("metaclass")
        } else if node.datatype {
            Some("datatype")
        } else if node.kind == NodeKind::Resource {
            Some("external")
        } else {
            None
        }
    }

    fn enumeration(&self, node: &Node) -> Option<&'a [Node]> {
        self.shorthand
            .plan
            .enumerations
            .iter()
            .find(|e| e.class.id == node.id)
            .map(|e| e.literals)
    }

    fn members(&mut self, node: &'a Node) {
        let mut fields = String::new();
        for literal in self.enumeration(node).unwrap_or_default() {
            self.words.literal(node, literal);
            fields.push_str(&format!(
                "  {{field}} {}\n",
                Escaped::inline(&literal.label)
            ));
        }
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
            self.words.attribute(
                node,
                &attribute.node.label,
                &types,
                &Qualifiers { multiplicity: None, tags: &[] },
            );
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
        let mut constraints = String::new();
        for (_, notation) in self
            .shorthand
            .restrictions
            .iter()
            .chain(&self.shorthand.neighbours)
            .filter(|(r, _)| r.class.id == node.id)
        {
            notation.describe(node, &mut self.words);
            fields.extend(notation.member(&self.identifiers));
            constraints.extend(notation.constraint(&self.identifiers));
        }
        constraint_projection::members(
            &mut fields,
            &mut constraints,
            node,
            self.diagram,
            &self.identifiers,
            &mut self.words,
        );
        // Constraint rows follow the attributes. A PlantUML separator would
        // be emitted inside the preceding member's link and break hit regions.
        self.output.push_str(&fields);
        self.output.push_str(&constraints);
    }
}
