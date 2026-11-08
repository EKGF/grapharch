//! A bounded, asserted OWL neighborhood retained as typed publication content.
mod constraint_projection;
mod constraints;
mod display;
mod existential;
mod label_lines;
mod labels;
mod layout;
mod projection;
pub(crate) use projection::Projection;
mod read;
mod shorthand;
pub(crate) mod svg;
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

pub(crate) const LANGUAGE: &str = "grapharch-class-diagram";
pub(crate) const LEGEND: &str =
    "Class diagrams use UML shorthand for asserted OWL facts and declared \
     SHACL constraints, without inference or a claim that validation passed. \
     RDF properties remain independent resources even when drawn as class \
     members or labelled associations. Domain/range declarations alone do not \
     impose requiredness or cardinality. Multiple domains or ranges apply \
     conjunctively, not as alternatives. A dotted rdfs:Resource source box is \
     a presentation placeholder when no specific domain is declared in the \
     selected inputs, not an asserted domain or a claim that none can be \
     entailed. Simple existential subclass restrictions use a class-scoped \
     association labelled at least one filler; this does not constrain all \
     property values or declare a global range. Other anonymous OWL \
     expressions remain explicit. Types and counts from simple property \
     shapes enrich the UML notation. Identical qualifiers are shown once; \
     where declaring shapes disagree, the disputed type or multiplicity is \
     omitted from the drawing and remains in the dedicated shape \
     documentation. No shape is chosen arbitrarily and conflicting counts are \
     not silently intersected. An absent count imposes no lower or upper \
     bound beyond zero or unbounded, respectively. Complex constraints \
     require the linked full shape definition. Original OWL facts and shape \
     provenance remain in the published source graph. Some diagrams show at \
     most five complete OWL relationship groups within an eighteen-fact \
     budget, selected in deterministic IRI order; omitted groups remain in \
     the published source graph. These views are not exhaustive.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    pub id:    String,
    pub label: String,
    pub kind:  String,
    pub link:  Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Edge {
    pub from:      Node,
    pub relation:  String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    pub to:        Node,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Diagram {
    pub focus:   Node,
    pub edges:   Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes:  Vec<constraints::Shape>,
    #[serde(default)]
    pub limited: bool,
}
impl Diagram {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 256 * 1024,
            "class diagram exceeds 256 KiB"
        );
        let diagram: Self = serde_json::from_str(source)?;
        ensure!(
            diagram.edges.len() <= 18,
            "class diagram exceeds eighteen relationship budget"
        );
        ensure!(
            diagram.shapes.len() <= 32 &&
                diagram.shapes.iter().map(|s| s.fields.len()).sum::<usize>() <=
                    128,
            "class diagram shape budget exceeded"
        );
        let mut identities = std::collections::BTreeMap::new(); // bounded: at most thirty-seven nodes.
        for node in diagram.nodes() {
            if let Some(previous) = identities.insert(&node.id, node) {
                ensure!(
                    previous == node,
                    "conflicting class diagram node identity"
                );
            }
            ensure!(
                node.label.len() <= 512 &&
                    node.id.len() <= 2048 &&
                    node.kind.len() <= 64,
                "class diagram node exceeds text budget"
            );
        }
        for edge in &diagram.edges {
            if let Some(predicate) = &edge.predicate {
                ensure!(
                    predicate.len() <= 2048,
                    "diagram predicate exceeds IRI budget"
                );
                oxrdf::NamedNode::new(predicate.clone())?;
            }
            ensure!(
                edge.relation.len() <= 256,
                "class diagram relation exceeds text budget"
            );
        }
        Ok(diagram)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        std::iter::once(&self.focus)
            .chain(self.edges.iter().flat_map(|e| [&e.from, &e.to]))
            .chain(self.shapes.iter().flat_map(|s| s.nodes()))
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        std::iter::once(&mut self.focus)
            .chain(self.edges.iter_mut().flat_map(|e| [&mut e.from, &mut e.to]))
            .chain(self.shapes.iter_mut().flat_map(|s| s.nodes_mut()))
    }
}
pub(super) use read::{Views, append};
