//! Configuration of an external RDF4J-compatible store.
use {crate::publication::sparql::HttpPolicy, serde::Deserialize};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SparqlStoreConfig {
    pub query_endpoint:        String,
    pub transactions_endpoint: String,
    #[serde(default)]
    pub http:                  HttpPolicy,
}
