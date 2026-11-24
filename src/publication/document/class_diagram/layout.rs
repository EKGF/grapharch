//! Semantic placement hints: inheritance is vertical, associations lateral.
use super::{
    Diagram,
    Edge,
    NodeKind,
    axiom_plan::AxiomPlan,
    restriction::Scope,
};

const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";

pub(super) struct Layout<'a> {
    /// Every drawn child-parent pair: asserted and axiom generalizations.
    parents:   Vec<(&'a str, &'a str)>, /* bounded: eighteen facts plus
                                         * axiom parts. */
    hierarchy: bool,
}

impl<'a> Layout<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let mut parents: Vec<(&str, &str)> = diagram
            .edges
            .iter()
            .filter(|e| e.predicate.as_deref() == Some(SUBCLASS))
            .map(|e| (e.from.id.as_str(), e.to.id.as_str()))
            .collect(); // bounded: eighteen retained facts.
        let plan = AxiomPlan::new(&diagram.axioms, Scope::Focus);
        let hierarchy = diagram.edges.iter().any(is_generalization) ||
            !plan.generalizations.is_empty();
        parents.extend(
            plan.generalizations
                .iter()
                .map(|g| (g.child.id.as_str(), g.parent.id.as_str())),
        );
        Self { parents, hierarchy }
    }

    pub fn has_hierarchy(&self) -> bool { self.hierarchy }

    pub fn generalization_arrow(
        &self,
        child: &str,
        parent: &str,
    ) -> &'static str {
        // Mutually subclassed resources cannot both sit above one another.
        // Preserve their asserted arrows without contradictory ranking hints.
        if self.reaches(parent, child) {
            "--|>"
        } else {
            "-up-|>"
        }
    }

    pub fn association_arrow(&self, from: &str, to: &str) -> &'static str {
        if !self.has_hierarchy() {
            return "-->";
        }
        // An unconstrained edge has its own default downward rank. When it
        // shares inheritance endpoints that rank can win over the triangle.
        // Align every edge with the known hierarchy rather than leaving two
        // contradictory rank requests for the layout engine to reconcile.
        match (self.reaches(from, to), self.reaches(to, from)) {
            (true, false) => "-up->",
            (false, true) => "-down->",
            // Cycles and self-relations have no vertical order.
            (true, true) => "-->",
            (false, false) => "-right->",
        }
    }

    /// An undirected line placed like an association.
    pub fn line(&self, from: &str, to: &str, dashed: bool) -> String {
        let arrow = self.association_arrow(from, to).trim_end_matches('>');
        if dashed {
            arrow.replace('-', ".")
        } else {
            arrow.to_owned()
        }
    }

    pub fn relation_arrow(&self, edge: &Edge) -> &'static str {
        if edge.predicate.as_deref() == Some(SUBCLASS) && self.has_hierarchy() {
            if self.reaches(&edge.to.id, &edge.from.id) {
                "-->"
            } else {
                "-up->"
            }
        } else {
            self.association_arrow(&edge.from.id, &edge.to.id)
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
            pending.extend(
                self.parents
                    .iter()
                    .filter(|(child, _)| *child == node)
                    .map(|(_, parent)| *parent),
            );
        }
        false
    }
}

pub(super) fn is_generalization(edge: &Edge) -> bool {
    edge.predicate.as_deref() == Some(SUBCLASS) &&
        matches!(
            edge.from.kind,
            NodeKind::Class | NodeKind::Resource
        ) &&
        matches!(edge.to.kind, NodeKind::Class | NodeKind::Resource)
}
