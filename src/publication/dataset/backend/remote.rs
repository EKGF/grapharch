//! RDF4J transaction protocol, with verified predicate-conflict isolation.
//!
//! One pooled `reqwest` client per store carries every request — begin,
//! query, update, statements, commit and rollback — and each response body is
//! read to its end so its connection returns to the pool.
mod commit;
mod config;
mod health;
mod isolation;
mod pattern;
mod quads;
mod reader;
mod session;
mod store;
#[cfg(test)]
mod tests;

pub use config::SparqlStoreConfig;
pub(super) use pattern::quad_pattern;
pub(crate) use {session::RemoteSession, store::RemoteStore};
