//! Simple OWL class restrictions mapped to UML members, associations and
//! class constraints; boolean and nested expressions stay explicit.
mod form;
mod notation;
mod parse;
mod vocabulary;
mod write;

pub(super) use {
    form::Restriction,
    notation::{Notation, Scope},
    write::Prohibition,
};
