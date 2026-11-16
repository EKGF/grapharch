//! Repeated anonymous RDF values are arranged by their content: `sh:order`
//! first, then their `sh:name`/`sh:message` text, then a digest of their
//! canonical description. Skolem identities depend on the source revision and
//! parser encounter order, so they never decide presentation order.

mod key;
mod order;

pub(in crate::publication::document) use order::AnonymousOrder;
