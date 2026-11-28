//! The mock RDF4J endpoint's lifecycle and test controls.
use {
    super::{http::serve, state::State},
    anyhow::Result,
    oxigraph::store::Store,
    std::{
        collections::HashMap,
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    },
};
pub struct Server {
    pub base: String,
    address:  SocketAddr,
    state:    Arc<Mutex<State>>,
    stop:     Arc<AtomicBool>,
    handle:   Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(serializable: bool) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let base = format!("http://{address}");
        let state = Arc::new(Mutex::new(State {
 store: Store::new()?, generation: 0, next: 0,
 transactions: HashMap::new(), serializable, source_status: 200,
 source: "<urn:ontology> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .".into(),
 bad_location: false, lose_commit: false, source_delay: 0, authenticated: false,
 connections: 0, requests: 0,
}));
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let shared = state.clone();
        let origin = base.clone();
        // A blocking accept answers each connection as soon as it arrives;
        // `Drop` wakes it with one loopback connection after setting `stop`.
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                if running.load(Ordering::SeqCst) {
                    break;
                }
                let stream = stream.expect("mock SPARQL endpoint accept");
                let state = shared.clone();
                let base = origin.clone();
                thread::spawn(move || {
                    if let Err(error) = serve(stream, &base, &state) &&
                        !error.downcast_ref::<std::io::Error>().is_some_and(
                            |e| {
                                matches!(
                                    e.kind(),
                                    std::io::ErrorKind::BrokenPipe |
                                        std::io::ErrorKind::ConnectionReset |
                                        std::io::ErrorKind::ConnectionAborted
                                )
                            },
                        )
                    {
                        panic!("{error:#}");
                    }
                });
            }
        });
        Ok(Self { base, address, state, stop, handle: Some(handle) })
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

    /// Connections accepted and requests answered so far; a client that
    /// reuses connections keeps the first far below the second.
    pub fn traffic(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap();
        (state.connections, state.requests)
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
        // Wake the blocking accept; a refused connection means it has
        // already stopped.
        let _ = TcpStream::connect(self.address);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}
