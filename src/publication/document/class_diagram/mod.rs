//! A bounded, asserted OWL neighborhood retained as typed publication content.
mod read;
pub(crate) mod svg;
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

pub(crate) const LANGUAGE: &str = "grapharch-class-diagram";
pub(crate) const LEGEND: &str =
    "Asserted OWL relationships only; no inference or required fields are \
     implied. RDF list membership is expanded for union/intersection \
     expressions. Properties are independent resources. Multiple domains and \
     ranges apply together, not as alternatives.";

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
    pub from:     Node,
    pub relation: String,
    pub to:       Node,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Diagram {
    pub focus: Node,
    pub edges: Vec<Edge>,
}
impl Diagram {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 64 * 1024,
            "class diagram exceeds 64 KiB"
        );
        let diagram: Self = serde_json::from_str(source)?;
        ensure!(
            diagram.edges.len() <= 6,
            "class diagram exceeds six relationship panel budget"
        );
        let mut identities = std::collections::BTreeMap::new(); // bounded: thirteen nodes.
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
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        std::iter::once(&mut self.focus)
            .chain(self.edges.iter_mut().flat_map(|e| [&mut e.from, &mut e.to]))
    }
}
pub(super) use read::append;
