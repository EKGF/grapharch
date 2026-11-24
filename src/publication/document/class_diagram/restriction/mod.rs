//! OWL class restrictions mapped to UML members, associations and class
//! constraints.
mod convert;
mod form;
mod notation;
mod write;

pub(super) use {
    form::{Constraint, Restriction},
    notation::{Notation, Scope},
    write::Prohibition,
};
