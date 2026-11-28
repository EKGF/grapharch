//! One RDF4J transaction: buffered inserts, queries, updates and rollback.
use {
    super::{reader::SessionReader, store::RemoteStore},
    crate::publication::sparql::{Settle, http_error},
    oxigraph::sparql::{
        QueryResults,
        results::{QueryResultsFormat, QueryResultsParser},
    },
    oxrdf::{Quad, QuadRef},
    std::{
        io,
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
    },
};

#[derive(Clone)]
pub(crate) struct RemoteSession(pub(super) Arc<SessionInner>);
pub(super) struct SessionInner {
    pub(super) store:  Arc<RemoteStore>,
    pub(super) url:    url::Url,
    pub(super) closed: AtomicBool,
    pending:           Mutex<Vec<Quad>>,
}
impl RemoteSession {
    pub(super) fn new(store: Arc<RemoteStore>, url: url::Url) -> Self {
        Self(Arc::new(SessionInner {
            store,
            url,
            closed: AtomicBool::new(false),
            pending: Mutex::new(Vec::new()),
        }))
    }

    pub(super) fn request(
        &self,
        action: &str,
    ) -> io::Result<reqwest::blocking::RequestBuilder> {
        self.0.store.healthy()?;
        if self.0.closed.load(Ordering::SeqCst) {
            return Err(io::Error::other("external transaction is closed"));
        }
        Ok(self
            .0
            .store
            .client
            .put(self.0.url.clone())
            .query(&[("action", action), ("infer", "false")]))
    }

    pub fn query(&self, query: &str) -> io::Result<QueryResults<'static>> {
        self.flush()?;
        let response = self
            .request("QUERY")?
            .header("Accept", "application/sparql-results+json")
            .header("Content-Type", "application/sparql-query")
            .body(query.to_owned())
            .send()
            .map_err(http_error)?;
        let reader = SessionReader::expecting(
            response,
            "application/sparql-results+json",
            "SPARQL query did not return SPARQL Results JSON",
            self,
        )?;
        QueryResultsParser::from_format(QueryResultsFormat::Json)
            .for_reader(reader)
            .map(Into::into)
            .map_err(io::Error::other)
    }

    pub fn update(&self, query: &str) -> io::Result<()> {
        self.flush()?;
        self.update_now(query)
    }

    fn update_now(&self, query: &str) -> io::Result<()> {
        if query.len() > 2 * 1024 * 1024 {
            return Err(io::Error::other(
                "SPARQL update exceeds 2 MiB request limit",
            ));
        }
        self.request("UPDATE")?
            .header("Content-Type", "application/sparql-update")
            .body(query.to_owned())
            .send()
            .map_err(http_error)?
            .settle()
    }

    pub fn insert(&self, quad: QuadRef<'_>) -> io::Result<()> {
        let mut pending = self.pending()?;
        let size: usize =
            pending.iter().map(|q| q.to_string().len() + 32).sum();
        if !pending.is_empty() &&
            (pending.len() >= 256 ||
                size + quad.to_string().len() > 512 * 1024)
        {
            drop(pending);
            self.flush()?;
            pending = self.pending()?;
        }
        if quad.to_string().len() > 1024 * 1024 {
            return Err(io::Error::other(
                "remote quad exceeds 1 MiB insert limit",
            ));
        }
        pending.push(quad.into_owned());
        Ok(())
    }

    fn pending(&self) -> io::Result<std::sync::MutexGuard<'_, Vec<Quad>>> {
        self.0
            .pending
            .lock()
            .map_err(|_| io::Error::other("remote insert buffer was poisoned"))
    }

    pub(super) fn flush(&self) -> io::Result<()> {
        let mut pending = self.pending()?;
        if pending.is_empty() {
            return Ok(());
        }
        let mut update = String::from("INSERT DATA {");
        for quad in pending.iter() {
            update.push_str(&super::quad_pattern(quad.as_ref()));
        }
        update.push('}');
        self.update_now(&update)?;
        pending.clear();
        Ok(())
    }

    pub fn rollback(&self) -> io::Result<()> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.0
            .store
            .client
            .delete(self.0.url.clone())
            .send()
            .map_err(http_error)?
            .settle()?;
        self.0.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}
