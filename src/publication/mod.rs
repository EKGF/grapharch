//! Graph-backed publication definitions, independent of output placement.

pub mod assembly;
pub mod cli;
pub mod collect;
pub mod dataset;
mod definitions;
mod diagnostic_graph;
pub mod diagnostics;
pub mod document;
mod gallery;
pub mod governance;
mod limits;
mod markdown;
mod mermaid;
mod native_html;
pub mod ontology_family;
mod ontology_specification;
pub mod plan;
pub mod render;
pub mod scope;
pub mod showcases;
mod showcases_dprod;
mod showcases_semantic;
mod validation;
pub mod view;
pub mod website;
pub mod writer;

pub use {
    definitions::{Renderer, Target},
    validation::validate,
};

/// Namespace shared by normalized instructions, fragments, and publication
/// plans.
pub const NAMESPACE: &str = "https://ekgf.org/ontology/grapharch#";
/// RDF type predicate.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Construct a vocabulary IRI from a trusted local term name.
pub fn term(local: &str) -> oxrdf::NamedNode {
    oxrdf::NamedNode::new_unchecked(format!("{NAMESPACE}{local}"))
}

pub mod sparql;

pub(crate) mod image_assets;
