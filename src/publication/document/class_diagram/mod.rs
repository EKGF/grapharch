//! A bounded, asserted OWL neighborhood retained as typed publication content.
mod constraint_projection;
mod constraints;
mod context;
mod context_read;
mod datatype;
mod display;
mod escape;
mod expression_label;
mod kind;
mod label_lines;
mod labels;
mod layout;
mod legend;
mod model;
mod multiplicity;
mod nested_existence;
mod ontology;
mod plantuml_source;
mod projection;
mod property_declaration;
mod read;
mod restriction;
mod shorthand;
pub(crate) mod svg;
mod union_domain;

pub(super) use read::{Views, append};
pub(crate) use {
    kind::NodeKind,
    legend::{LANGUAGE, LEGEND},
    model::{Diagram, Edge, Node},
    projection::Projection,
};
