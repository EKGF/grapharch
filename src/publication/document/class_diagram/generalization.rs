//! Generalizations with their generalization-set constraints.
//!
//! Disjoint classes that share a drawn parent mark both generalizations
//! `{disjoint}`; a union a class is equivalent to marks its members'
//! generalizations `{complete}`. Disjointness without a shared drawn parent
//! remains a dashed `{disjoint}` constraint line.
use {
    super::{Diagram, Node, axiom_plan::AxiomPlan, layout},
    std::collections::BTreeSet,
};
const DISJOINT: &str = "http://www.w3.org/2002/07/owl#disjointWith";

/// One drawn generalization.
pub(super) struct Arrow<'a> {
    pub child:  &'a Node,
    pub parent: &'a Node,
    pub tags:   Vec<&'static str>, // bounded: two set constraints.
}
/// Every generalization and remaining disjointness of one diagram.
pub(super) struct Generalizations<'a> {
    pub arrows:   Vec<Arrow<'a>>,
    /// Disjoint pairs without a shared drawn parent.
    pub disjoint: Vec<(&'a Node, &'a Node)>,
    /// Indices of every edge drawn here.
    pub drawn:    BTreeSet<usize>, // bounded: eighteen edge indices.
}
impl<'a> Generalizations<'a> {
    pub fn new(
        diagram: &'a Diagram,
        plan: &AxiomPlan<'a>,
        consumed: &BTreeSet<usize>,
    ) -> Self {
        let mut result = Self {
            arrows:   Vec::new(),
            disjoint: Vec::new(),
            drawn:    BTreeSet::new(),
        };
        for (index, edge) in diagram.edges.iter().enumerate() {
            if !consumed.contains(&index) && layout::is_generalization(edge) {
                result.drawn.insert(index);
                result.add(&edge.from, &edge.to, None);
            }
        }
        for generalization in &plan.generalizations {
            result.add(
                generalization.child,
                generalization.parent,
                generalization.complete.then_some("complete"),
            );
        }
        for (index, edge) in diagram.edges.iter().enumerate() {
            if consumed.contains(&index) ||
                edge.predicate.as_deref() != Some(DISJOINT) ||
                !edge.from.kind.is_class_like() ||
                !edge.to.kind.is_class_like()
            {
                continue;
            }
            result.drawn.insert(index);
            if !result.separate(&edge.from, &edge.to) &&
                !result.disjoint.iter().any(|(a, b)| {
                    (a.id == edge.from.id && b.id == edge.to.id) ||
                        (a.id == edge.to.id && b.id == edge.from.id)
                })
            {
                result.disjoint.push((&edge.from, &edge.to));
            }
        }
        result
    }

    fn add(
        &mut self,
        child: &'a Node,
        parent: &'a Node,
        tag: Option<&'static str>,
    ) {
        let arrow = match self
            .arrows
            .iter_mut()
            .find(|a| a.child.id == child.id && a.parent.id == parent.id)
        {
            Some(arrow) => arrow,
            None => {
                self.arrows.push(Arrow { child, parent, tags: Vec::new() });
                self.arrows.last_mut().unwrap_or_else(|| unreachable!())
            },
        };
        if let Some(tag) = tag &&
            !arrow.tags.contains(&tag)
        {
            arrow.tags.push(tag);
        }
    }

    /// Mark both generalizations to every shared parent `{disjoint}`;
    /// whether any parent is shared.
    fn separate(&mut self, first: &Node, second: &Node) -> bool {
        let parents: Vec<&'a str> = self
            .arrows
            .iter()
            .filter(|a| a.child.id == first.id)
            .map(|a| a.parent.id.as_str())
            .filter(|p| {
                self.arrows
                    .iter()
                    .any(|a| a.child.id == second.id && a.parent.id == *p)
            })
            .collect(); // bounded: the drawn arrows.
        for arrow in &mut self.arrows {
            if (arrow.child.id == first.id || arrow.child.id == second.id) &&
                parents.contains(&arrow.parent.id.as_str()) &&
                !arrow.tags.contains(&"disjoint")
            {
                arrow.tags.push("disjoint");
            }
        }
        !parents.is_empty()
    }
}
