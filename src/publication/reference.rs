//! One decide-then-act stage for every reference an authored source makes.
//!
//! [`ReferenceForm`] records how a reference was written; the pure,
//! table-driven [`ReferenceClassifier`] decides what it points at
//! ([`ReferenceKind`]) from a [`ReferenceLookup`] of the publication; renderers
//! only turn the resulting classification into output, and the
//! [`ReferenceRecorder`] publishes every decision as RDF.
mod classifier;
mod form;
mod kind;
mod record;
mod tables;

pub(crate) use {
    classifier::{
        LinkPolicy,
        LookupPage,
        ReferenceClassifier,
        ReferenceLookup,
    },
    kind::ReferenceKind,
    record::{FILE as RECORD_FILE, ReferenceRecorder, graph},
    tables::badge as is_badge,
};
#[cfg(test)]
use {
    form::ReferenceForm,
    kind::{Classification, ForgeObject},
};

#[cfg(test)]
mod tests;
