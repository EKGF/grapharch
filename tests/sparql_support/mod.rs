#![allow(dead_code)]
use {
    anyhow::Result,
    oxigraph::{
        sparql::{
            QueryResults,
            SparqlEvaluator,
            results::{QueryResultsFormat, QueryResultsSerializer},
        },
        store::Store,
    },
    std::{
        collections::HashMap,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    },
};
struct Tx {
    store:      Store,
    generation: u64,
    dirty:      bool,
}
struct State {
    store:         Store,
    generation:    u64,
    next:          u64,
    transactions:  HashMap<u64, Tx>,
    serializable:  bool,
    source_status: u16,
    source:        String,
    bad_location:  bool,
    lose_commit:   bool,
    source_delay:  u64,
    authenticated: bool,
}
pub struct Server {
    pub base: String,
    state:    Arc<Mutex<State>>,
    stop:     Arc<AtomicBool>,
    handle:   Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(serializable: bool) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let base = format!("http://{}", listener.local_addr()?);
        listener.set_nonblocking(true)?;
        let state = Arc::new(Mutex::new(State {
 store: Store::new()?, generation: 0, next: 0,
 transactions: HashMap::new(), serializable, source_status: 200,
 source: "<urn:ontology> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .".into(),
 bad_location: false, lose_commit: false, source_delay: 0, authenticated: false,
}));
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let shared = state.clone();
        let origin = base.clone();
        let handle = thread::spawn(move || {
            while !running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let state = shared.clone();
                        let base = origin.clone();
                        thread::spawn(move || {
                            if let Err(error) = serve(stream, &base, state)
                                && !error.downcast_ref::<std::io::Error>().is_some_and(|e| matches!(e.kind(), std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset)) { panic!("{error:#}"); }
                        });
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(1))
                    },
                    Err(e) => panic!("{e}"),
                }
            }
        });
        Ok(Self { base, state, stop, handle: Some(handle) })
    }

    pub fn source(&self, status: u16, body: &str) {
        let mut state = self.state.lock().unwrap();
        state.source_status = status;
        state.source = body.into();
    }

    pub fn slow_source(&self) {
        self.state.lock().unwrap().source_delay = 1200;
    }

    pub fn authenticated(&self) -> bool {
        self.state.lock().unwrap().authenticated
    }

    pub fn lose_commit(&self) { self.state.lock().unwrap().lose_commit = true; }

    pub fn bad_location(&self) {
        self.state.lock().unwrap().bad_location = true;
    }

    pub fn transaction_count(&self) -> usize {
        self.state.lock().unwrap().transactions.len()
    }

    pub fn config(&self) -> grapharch::publication::dataset::SparqlStoreConfig {
        serde_yaml::from_str(&format!(
            "query_endpoint: {}/repositories/test\ntransactions_endpoint: \
             {}/repositories/test/transactions\nhttp:\n  timeout_seconds: 5\n",
            self.base, self.base
        ))
        .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}
fn serve(
    mut stream: TcpStream,
    base: &str,
    state: Arc<Mutex<State>>,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
        anyhow::ensure!(bytes.len() < 65536, "headers too large");
    };
    let headers = String::from_utf8(bytes.clone())?;
    let mut first = headers.lines().next().unwrap().split_whitespace();
    let method = first.next().unwrap();
    let path = first.next().unwrap();
    let length = headers
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    anyhow::ensure!(length <= 2 * 1024 * 1024, "request too large");
    bytes.resize(header_end + length, 0);
    stream.read_exact(&mut bytes[header_end..])?;
    let body = String::from_utf8(bytes[header_end..].to_vec())?;
    let url = url::Url::parse(&format!("{base}{path}"))?;
    let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let delay = if url.path() == "/source" {
        state.lock().unwrap().source_delay
    } else {
        0
    };
    if delay > 0 {
        thread::sleep(std::time::Duration::from_millis(delay));
    }
    let mut state = state.lock().unwrap();
    state.authenticated |= headers
        .to_ascii_lowercase()
        .contains("authorization: bearer grapharch-test-only");
    let (status, mime, body, location) = if url.path() == "/source" {
        (
            state.source_status,
            "application/n-triples",
            state.source.clone(),
            None,
        )
    } else if method == "POST" && url.path().ends_with("/transactions") {
        anyhow::ensure!(
            body.contains("isolation-level=SERIALIZABLE"),
            "missing isolation request"
        );
        state.next += 1;
        let id = state.next;
        let store = Store::new()?;
        for q in state.store.iter() {
            store.insert(q?.as_ref())?;
        }
        let generation = state.generation;
        state
            .transactions
            .insert(id, Tx { store, generation, dirty: false });
        let location = if state.bad_location {
            format!("{base}/other/{id}")
        } else {
            format!("{base}/repositories/test/transactions/{id}")
        };
        (201, "text/plain", String::new(), Some(location))
    } else if let Some(id) = url
        .path()
        .strip_prefix("/repositories/test/transactions/")
        .and_then(|x| x.parse::<u64>().ok())
    {
        if method == "DELETE" {
            state.transactions.remove(&id);
            (204, "text/plain", String::new(), None)
        } else {
            let action = params.get("action").map(String::as_str).unwrap_or("");
            match action {
                "QUERY" => {
                    let tx = state.transactions.get(&id).unwrap();
                    let output = query_json(&tx.store, &body)?;
                    (
                        200,
                        "application/sparql-results+json",
                        String::from_utf8(output)?,
                        None,
                    )
                },
                "UPDATE" => {
                    let tx = state.transactions.get_mut(&id).unwrap();
                    tx.store.update(&body)?;
                    tx.dirty = true;
                    (204, "text/plain", String::new(), None)
                },
                "GET" => {
                    let tx = state.transactions.get(&id).unwrap();
                    let mut output = String::new();
                    for q in tx.store.iter() {
                        let q = q?;
                        if params
                            .get("subj")
                            .is_some_and(|v| v != &q.subject.to_string()) ||
                            params.get("pred").is_some_and(|v| {
                                v != &q.predicate.to_string()
                            }) ||
                            params
                                .get("obj")
                                .is_some_and(|v| v != &q.object.to_string()) ||
                            params.get("context").is_some_and(|v| {
                                if v == "null" {
                                    q.graph_name !=
                                        oxrdf::GraphName::DefaultGraph
                                } else {
                                    v != &q.graph_name.to_string()
                                }
                            })
                        {
                            continue;
                        }
                        output.push_str(&format!("{q} .\n"));
                    }
                    (200, "application/n-quads", output, None)
                },
                "COMMIT" => {
                    let tx = state.transactions.get(&id).unwrap();
                    if state.serializable &&
                        tx.dirty &&
                        tx.generation != state.generation
                    {
                        (
                            500,
                            "text/plain",
                            "Transaction handling error: SailConflictException"
                                .into(),
                            None,
                        )
                    } else {
                        let tx = state.transactions.remove(&id).unwrap();
                        if tx.dirty {
                            state.store = tx.store;
                            state.generation += 1;
                        }
                        if state.lose_commit {
                            state.lose_commit = false;
                            return Ok(());
                        }
                        (204, "text/plain", String::new(), None)
                    }
                },
                _ => {
                    (
                        400,
                        "text/plain",
                        "unsupported action".into(),
                        None,
                    )
                },
            }
        }
    } else if url.path() == "/repositories/test" && method == "POST" {
        let output = query_json(&state.store, &body)?;
        (
            200,
            "application/sparql-results+json",
            String::from_utf8(output)?,
            None,
        )
    } else {
        (404, "text/plain", "not found".into(), None)
    };
    let location = location
        .map(|v| format!("Location: {v}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "HTTP/1.1 {status} Mock\r\nContent-Type: {mime}\r\nContent-Length: \
         {}\r\nConnection: close\r\n{location}\r\n{body}",
        body.len()
    )?;
    Ok(())
}

fn query_json(store: &Store, query: &str) -> Result<Vec<u8>> {
    let serializer =
        QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    match SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        QueryResults::Boolean(value) => {
            Ok(serializer.serialize_boolean_to_writer(Vec::new(), value)?)
        },
        QueryResults::Solutions(rows) => {
            let mut writer = serializer.serialize_solutions_to_writer(
                Vec::new(),
                rows.variables().to_vec(),
            )?;
            for row in rows {
                writer.serialize(row?.iter())?;
            }
            Ok(writer.finish()?)
        },
        QueryResults::Graph(_) => {
            anyhow::bail!("protocol mock requires SELECT or ASK")
        },
    }
}
