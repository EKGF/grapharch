//! A bounded, asserted OWL neighborhood retained as typed publication content.
mod axiom_plan;
mod constraint_projection;
mod constraints;
mod context;
mod context_read;
mod datatype;
mod display;
mod escape;
mod expression;
mod expression_label;
mod generalization;
mod kind;
mod label_lines;
mod labels;
mod layout;
mod legend;
mod metaclass;
mod model;
mod multiplicity;
mod ontology;
mod plantuml_source;
mod projection;
mod property_declaration;
mod read;
mod restriction;
mod shorthand;
mod sources;
pub(crate) mod svg;
mod union_domain;
mod verbalization;

pub(super) use read::{Views, append};
pub(crate) use {
    kind::NodeKind,
    legend::{LANGUAGE, LEGEND},
    model::{Diagram, Edge, Node},
    projection::Projection,
};
