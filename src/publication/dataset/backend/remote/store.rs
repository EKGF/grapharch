//! A connected external store and the transactions it begins.
use {
    super::{
        config::SparqlStoreConfig,
        health::{Hazard, Health},
        session::RemoteSession,
    },
    crate::publication::sparql::{Drain, endpoint, http_error},
    anyhow::{Result, ensure},
    reqwest::{StatusCode, blocking::Client},
    std::{io, sync::Arc},
};

pub(crate) struct RemoteStore {
    pub(super) config: SparqlStoreConfig,
    /// The one pooled client for every request to this store.
    pub(super) client: Client,
    health:            Health,
}
impl RemoteStore {
    pub fn connect(config: SparqlStoreConfig) -> Result<Arc<Self>> {
        let query = endpoint(&config.query_endpoint)?;
        let transactions = endpoint(&config.transactions_endpoint)?;
        ensure!(
            query.origin() == transactions.origin(),
            "query and RDF4J transaction endpoints must share one origin"
        );
        let client = config.http.blocking()?;
        let store = Arc::new(Self { config, client, health: Health::new() });
        store.verify_isolation()?;
        Ok(store)
    }

    pub(super) fn healthy(&self) -> io::Result<()> { self.health.check() }

    pub(super) fn record(&self, hazard: Hazard) { self.health.record(hazard); }

    pub fn session(self: &Arc<Self>) -> io::Result<RemoteSession> {
        self.healthy()?;
        let mut response = self
            .client
            .post(&self.config.transactions_endpoint)
            .form(&[("isolation-level", "SERIALIZABLE")])
            .send()
            .map_err(http_error)?;
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let code = response.status();
        response.drain()?;
        if code != StatusCode::CREATED {
            return Err(io::Error::other(format!(
                "external KG requires RDF4J SERIALIZABLE transactions; begin \
                 returned HTTP {code}"
            )));
        }
        let location = location.ok_or_else(|| {
            io::Error::other("RDF4J begin omitted transaction Location")
        })?;
        Ok(RemoteSession::new(
            self.clone(),
            self.transaction_url(&location)?,
        ))
    }

    /// Resolves a begin response's `Location` and refuses one that leaves
    /// the configured transactions endpoint.
    fn transaction_url(&self, location: &str) -> io::Result<url::Url> {
        let base = url::Url::parse(&self.config.transactions_endpoint)
            .map_err(io::Error::other)?;
        let url = base.join(location).map_err(io::Error::other)?;
        let prefix = format!("{}/", base.path().trim_end_matches('/'));
        if url.origin() != base.origin() ||
            !url.path().strip_prefix(&prefix).is_some_and(|id| {
                !id.is_empty() &&
                    id.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            }) ||
            url.query().is_some() ||
            url.fragment().is_some() ||
            !url.username().is_empty() ||
            url.password().is_some()
        {
            return Err(io::Error::other(
                "transaction Location escaped its configured repository \
                 endpoint",
            ));
        }
        Ok(url)
    }
}
