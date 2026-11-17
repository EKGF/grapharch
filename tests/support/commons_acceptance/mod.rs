//! Acceptance of the GraphArch Commons edition against OMG's formal
//! specification: structure, typography, terms and page images.
mod criteria;
mod criterion;
mod edition;
mod pages;
mod reference;
mod report;
mod style;
mod terms;

pub use {
    criteria::Acceptance,
    criterion::Criterion,
    edition::{Edition, Side},
    pages::{PageAnchor, PagePair},
    reference::OmgReference,
    report::AcceptanceReport,
};
