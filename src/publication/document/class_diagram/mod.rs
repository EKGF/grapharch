//! A bounded, asserted OWL neighborhood retained as typed publication content.
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
    "OWL neighborhood diagrams present asserted relationships without \
     inference. A property with one unambiguous declared domain may be drawn \
     as a class attribute or a labelled association; RDF properties remain \
     independent resources. Domain/range declarations do not impose \
     requiredness or cardinality. Multiple domains or ranges apply \
     conjunctively, not as alternatives. Anonymous OWL expressions remain \
     explicit. Relationships as text preserves the original assertions. Where \
     shown, SHACL validation views document each explicitly associated shape \
     separately; they do not add OWL axioms or report validation results.";

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
            diagram.edges.len() <= 18,
            "class diagram exceeds eighteen relationship budget"
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
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        std::iter::once(&mut self.focus)
            .chain(self.edges.iter_mut().flat_map(|e| [&mut e.from, &mut e.to]))
    }
}
pub(super) use read::append;
