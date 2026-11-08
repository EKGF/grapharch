//! Semantic placement hints: inheritance is vertical, associations lateral.
use super::{Diagram, Edge};

const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";

pub(super) struct Layout<'a> {
    diagram: &'a Diagram,
}

impl<'a> Layout<'a> {
    pub fn new(diagram: &'a Diagram) -> Self { Self { diagram } }

    pub fn has_hierarchy(&self) -> bool {
        self.diagram.edges.iter().any(is_generalization)
    }

    pub fn generalization_arrow(&self, edge: &Edge) -> &'static str {
        // Mutually subclassed resources cannot both sit above one another.
        // Preserve their asserted arrows without contradictory ranking hints.
        if self.reaches(&edge.to.id, &edge.from.id) {
            "--|>"
        } else {
            "-up-|>"
        }
    }

    pub fn association_arrow(&self, from: &str, to: &str) -> &'static str {
        if !self.has_hierarchy() ||
            self.reaches(from, to) ||
            self.reaches(to, from)
        {
            "-->"
        } else {
            "-right->"
        }
    }

    fn reaches(&self, from: &str, to: &str) -> bool {
        let mut pending = vec![from]; // bounded: at most thirty-seven diagram nodes.
        let mut visited = std::collections::BTreeSet::new(); // bounded: at most thirty-seven diagram nodes.
        while let Some(node) = pending.pop() {
            if node == to {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            for edge in
                self.diagram.edges.iter().filter(|edge| {
                    is_generalization(edge) && edge.from.id == node
                })
            {
                pending.push(&edge.to.id);
            }
        }
        false
    }
}

pub(super) fn is_generalization(edge: &Edge) -> bool {
    edge.predicate.as_deref() == Some(SUBCLASS) &&
        matches!(edge.from.kind.as_str(), "Class" | "Resource") &&
        matches!(edge.to.kind.as_str(), "Class" | "Resource")
}
