//! Repository awareness: what Git, the forge and the package manifests say
//! about the repository being documented, recorded as RDF.
//!
//! The model is DOAP for the software project, its repository and releases,
//! schema.org for dates, keywords and discussion links, SPDX license IRIs,
//! and `ga:CodePackage` for each package a manifest declares. See the
//! "Repository metadata" section of the ontology guide.
mod discoverer;
mod facts;
pub(crate) mod forge;
mod forge_api;
mod git_facts;
mod manifest;
pub(crate) mod package;
pub(crate) mod page_revisions;
mod record;
mod release_tags;

#[cfg(test)]
use facts::DiagnosticCode;
pub(crate) use {
    discoverer::RepositoryDiscoverer,
    facts::RepositoryFacts,
    forge_api::ForgeAccess,
    record::RepositoryRecord,
};

#[cfg(test)]
mod tests;
