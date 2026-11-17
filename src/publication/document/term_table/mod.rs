//! Standards-style tabular ontology reference: per ontology, a metadata table
//! of its header annotations and a details table of its terms, with source
//! annotations and axioms in description-logic notation. A profile opts in
//! with `ga:termTable`; the sectioned per-term reference stays the default.
mod annotations;
mod axioms;
mod bodies;
mod documentor;
mod expression;
mod kind;
mod layout;
mod markdown;
mod metadata;
mod names;
mod pattern;
mod quality;
mod rdf_list;
mod restriction;
mod row;

#[cfg(test)]
mod tests;

pub(crate) use {
    documentor::{OntologySource, TermTableDocumentor},
    layout::TermTable,
};
