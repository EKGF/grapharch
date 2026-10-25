//! RDF4J transaction protocol, with verified predicate-conflict isolation.
use {
    super::Quads,
    crate::publication::sparql::{
        HttpPolicy,
        LimitedRead,
        endpoint,
        http_error,
        status,
    },
    anyhow::{Result, ensure},
    oxigraph::sparql::{
        QueryResults,
        results::{QueryResultsFormat, QueryResultsParser},
    },
    oxrdf::{
        GraphNameRef,
        Literal,
        NamedNode,
        NamedNodeRef,
        NamedOrBlankNodeRef,
        Quad,
        QuadRef,
        TermRef,
    },
    oxrdfio::{RdfFormat, RdfParser},
    reqwest::{
        StatusCode,
        blocking::{Client, Response},
    },
    serde::Deserialize,
    std::{
        io::{self, Read},
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
    },
};
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SparqlStoreConfig {
    pub query_endpoint:        String,
    pub transactions_endpoint: String,
    #[serde(default)]
    pub http:                  HttpPolicy,
}
pub(crate) struct RemoteStore {
    config:    SparqlStoreConfig,
    client:    Client,
    uncertain: AtomicBool,
}
#[derive(Clone)]
pub(crate) struct RemoteSession(Arc<SessionInner>);
struct SessionInner {
    store:   Arc<RemoteStore>,
    url:     url::Url,
    closed:  AtomicBool,
    pending: Mutex<Vec<Quad>>,
}
#[derive(Debug)]
struct Conflict;
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("external SPARQL transaction had a serialization conflict")
    }
}
impl std::error::Error for Conflict {}
impl RemoteStore {
    pub fn connect(config: SparqlStoreConfig) -> Result<Arc<Self>> {
        let query = endpoint(&config.query_endpoint)?;
        let transactions = endpoint(&config.transactions_endpoint)?;
        ensure!(
            query.origin() == transactions.origin(),
            "query and RDF4J transaction endpoints must share one origin"
        );
        let client = config.http.blocking()?;
        let store = Arc::new(Self {
            config,
            client,
            uncertain: AtomicBool::new(false),
        });
        store.verify_isolation()?;
        Ok(store)
    }

    fn healthy(&self) -> io::Result<()> {
        if self.uncertain.load(Ordering::SeqCst) {
            Err(io::Error::other(
                "external commit outcome is unknown; reopen and inspect the \
                 dataset before further operations",
            ))
        } else {
            Ok(())
        }
    }

    pub fn session(self: &Arc<Self>) -> io::Result<RemoteSession> {
        self.healthy()?;
        let response = self
            .client
            .post(&self.config.transactions_endpoint)
            .form(&[("isolation-level", "SERIALIZABLE")])
            .send()
            .map_err(http_error)?;
        if response.status() != StatusCode::CREATED {
            return Err(io::Error::other(format!(
                "external KG requires RDF4J SERIALIZABLE transactions; begin \
                 returned HTTP {}",
                response.status()
            )));
        }
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                io::Error::other("RDF4J begin omitted transaction Location")
            })?;
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
        Ok(RemoteSession(Arc::new(SessionInner {
            store: self.clone(),
            url,
            closed: AtomicBool::new(false),
            pending: Mutex::new(Vec::new()),
        })))
    }

    fn verify_isolation(self: &Arc<Self>) -> Result<()> {
        let graph = NamedNode::new(format!(
            "urn:grapharch:isolation-probe:{}",
            uuid::Uuid::new_v4()
        ))?;
        let first = self.session()?;
        let second = self.session()?;
        let result = (|| -> Result<()> {
            let query = format!("ASK {{ GRAPH {graph} {{ ?s ?p ?o }} }}");
            for session in [&first, &second] {
                ensure!(
                    matches!(
                        session.query(&query)?,
                        QueryResults::Boolean(false)
                    ),
                    "isolation probe graph is not empty"
                );
            }
            let predicate =
                NamedNode::new("urn:grapharch:isolation-probe:predicate")?;
            first.insert(
                Quad::new(
                    NamedNode::new("urn:grapharch:isolation-probe:first")?,
                    predicate.clone(),
                    Literal::new_simple_literal("first"),
                    graph.clone(),
                )
                .as_ref(),
            )?;
            second.insert(
                Quad::new(
                    NamedNode::new("urn:grapharch:isolation-probe:second")?,
                    predicate,
                    Literal::new_simple_literal("second"),
                    graph.clone(),
                )
                .as_ref(),
            )?;
            // The writes are disjoint. SNAPSHOT permits both; SERIALIZABLE
            // must reject the predicate-range/phantom conflict.
            first.commit()?;
            // Both configured endpoints must address the same repository.
            let response = self
                .client
                .post(&self.config.query_endpoint)
                .header("Content-Type", "application/sparql-query")
                .header("Accept", "application/sparql-results+json")
                .body(query.clone())
                .send()
                .map_err(http_error)?;
            status(response.status())?;
            let result: QueryResults =
                QueryResultsParser::from_format(QueryResultsFormat::Json)
                    .for_reader(LimitedRead::new(
                        response,
                        self.config.http.max_response_bytes,
                    ))?
                    .into();
            ensure!(
                matches!(result, QueryResults::Boolean(true)),
                "query and transaction endpoints do not expose the same \
                 committed repository"
            );
            let rejected = second.commit().err().is_some_and(|e| {
                e.get_ref().is_some_and(|e| e.is::<Conflict>())
            });
            ensure!(
                rejected,
                "SPARQL backend did not demonstrate SERIALIZABLE \
                 predicate-conflict isolation"
            );
            Ok(())
        })();
        let rollback_first = first.rollback();
        let rollback_second = second.rollback();
        let cleanup = (|| -> io::Result<()> {
            let tx = self.session()?;
            tx.update(&format!("DROP SILENT GRAPH {graph}"))?;
            tx.commit()
        })();
        rollback_first?;
        rollback_second?;
        cleanup?;
        result
    }
}
impl RemoteSession {
    fn request(
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
        status(response.status())?;
        let mime = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !mime
            .split(';')
            .next()
            .is_some_and(|v| v.trim() == "application/sparql-results+json")
        {
            return Err(io::Error::other(
                "SPARQL query did not return SPARQL Results JSON",
            ));
        }
        let reader = SessionReader {
            response: LimitedRead::new(
                response,
                self.0.store.config.http.max_response_bytes,
            ),
            _session: self.clone(),
        };
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
        let response = self
            .request("UPDATE")?
            .header("Content-Type", "application/sparql-update")
            .body(query.to_owned())
            .send()
            .map_err(http_error)?;
        status(response.status())
    }

    pub fn insert(&self, quad: QuadRef<'_>) -> io::Result<()> {
        let mut pending = self.0.pending.lock().map_err(|_| {
            io::Error::other("remote insert buffer was poisoned")
        })?;
        let size: usize =
            pending.iter().map(|q| q.to_string().len() + 32).sum();
        if !pending.is_empty() &&
            (pending.len() >= 256 ||
                size + quad.to_string().len() > 512 * 1024)
        {
            drop(pending);
            self.flush()?;
            pending = self.0.pending.lock().map_err(|_| {
                io::Error::other("remote insert buffer was poisoned")
            })?;
        }
        if quad.to_string().len() > 1024 * 1024 {
            return Err(io::Error::other(
                "remote quad exceeds 1 MiB insert limit",
            ));
        }
        pending.push(quad.into_owned());
        Ok(())
    }

    fn flush(&self) -> io::Result<()> {
        let mut pending = self.0.pending.lock().map_err(|_| {
            io::Error::other("remote insert buffer was poisoned")
        })?;
        if pending.is_empty() {
            return Ok(());
        }
        let mut update = String::from("INSERT DATA {");
        for quad in pending.iter() {
            update.push_str(&quad_pattern(quad.as_ref()));
        }
        update.push('}');
        self.update_now(&update)?;
        pending.clear();
        Ok(())
    }

    pub fn commit(&self) -> io::Result<()> {
        self.flush()?;
        let response = match self.request("COMMIT")?.send() {
            Ok(response) => response,
            Err(error) => {
                self.0.store.uncertain.store(true, Ordering::SeqCst);
                return Err(http_error(error));
            },
        };
        let code = response.status();
        if code.is_success() {
            self.0.closed.store(true, Ordering::SeqCst);
            return Ok(());
        }
        let mut body = String::new();
        if let Err(error) =
            LimitedRead::new(response, 8192).read_to_string(&mut body)
        {
            self.0.store.uncertain.store(true, Ordering::SeqCst);
            return Err(error);
        }
        if code == StatusCode::CONFLICT ||
            (code.is_server_error() &&
                body.contains("SailConflictException"))
        {
            return Err(io::Error::other(Conflict));
        }
        if code.is_server_error() {
            self.0.store.uncertain.store(true, Ordering::SeqCst);
        }
        Err(io::Error::other(format!(
            "external commit failed with HTTP {code}; outcome may be unknown"
        )))
    }

    pub fn rollback(&self) -> io::Result<()> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let response = self
            .0
            .store
            .client
            .delete(self.0.url.clone())
            .send()
            .map_err(http_error)?;
        status(response.status())?;
        self.0.closed.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn quads_for_pattern(
        &self,
        s: Option<NamedOrBlankNodeRef<'_>>,
        p: Option<NamedNodeRef<'_>>,
        o: Option<TermRef<'_>>,
        g: Option<GraphNameRef<'_>>,
    ) -> Quads<'static> {
        let result = (|| -> io::Result<_> {
            self.flush()?;
            let mut params = Vec::new();
            if let Some(value) = s {
                params.push(("subj", value.to_string()));
            }
            if let Some(value) = p {
                params.push(("pred", value.to_string()));
            }
            if let Some(value) = o {
                params.push(("obj", value.to_string()));
            }
            if let Some(value) = g {
                params.push((
                    "context",
                    if value == GraphNameRef::DefaultGraph {
                        "null".into()
                    } else {
                        value.to_string()
                    },
                ));
            }
            let response = self
                .request("GET")?
                .query(&params)
                .header("Accept", "application/n-quads")
                .send()
                .map_err(http_error)?;
            status(response.status())?;
            let mime = response
                .headers()
                .get("Content-Type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !mime
                .split(';')
                .next()
                .is_some_and(|v| v.trim() == "application/n-quads")
            {
                return Err(io::Error::other(
                    "RDF4J statements response must preserve contexts as \
                     N-Quads",
                ));
            }
            let reader = SessionReader {
                response: LimitedRead::new(
                    response,
                    self.0.store.config.http.max_response_bytes,
                ),
                _session: self.clone(),
            };
            Ok(RdfParser::from_format(RdfFormat::NQuads).for_reader(reader))
        })();
        match result {
            Ok(parser) => Box::new(parser.map(|q| q.map_err(io::Error::other))),
            Err(e) => Box::new(std::iter::once(Err(e))),
        }
    }
}
struct SessionReader {
    response: LimitedRead<Response>,
    _session: RemoteSession,
}
impl Read for SessionReader {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.response.read(b)
    }
}
impl Drop for SessionInner {
    fn drop(&mut self) {
        if !self.closed.load(Ordering::SeqCst) {
            match self.store.client.delete(self.url.clone()).send() {
                Ok(r) if r.status().is_success() => {},
                Ok(r) => {
                    tracing::error!(status=%r.status(),"could not roll back abandoned external transaction");
                    self.store.uncertain.store(true, Ordering::SeqCst);
                },
                Err(_) => {
                    tracing::error!(
                        "could not roll back abandoned external transaction"
                    );
                    self.store.uncertain.store(true, Ordering::SeqCst);
                },
            }
        }
    }
}
pub(super) fn quad_pattern(quad: QuadRef<'_>) -> String {
    let triple = format!(
        "{} {} {} .",
        quad.subject, quad.predicate, quad.object
    );
    match quad.graph_name {
        GraphNameRef::DefaultGraph => triple,
        graph => format!("GRAPH {graph} {{ {triple} }}"),
    }
}
