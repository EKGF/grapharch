//! One persistent HTTP/1.1 connection to the mock RDF4J endpoint.
use {
    super::{
        query::query_json,
        state::{State, Tx},
    },
    anyhow::Result,
    oxigraph::store::Store,
    std::{
        collections::HashMap,
        io::{Read, Write},
        net::TcpStream,
        sync::Mutex,
        thread,
    },
};
/// Serves HTTP/1.1 requests on one persistent connection until the client
/// closes it.
///
/// Closing after every response made each SPARQL call a fresh loopback
/// connection, and the TIME_WAIT entries those closes leave behind exhausted
/// the Windows dynamic port range (`os error 10048`) during long tests.
pub(super) fn serve(
    mut stream: TcpStream,
    base: &str,
    state: &Mutex<State>,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    state.lock().unwrap().connections += 1;
    while respond(&mut stream, base, state)? {}
    Ok(())
}
/// Answers one request; `false` means the connection is finished.
fn respond(
    stream: &mut TcpStream,
    base: &str,
    state: &Mutex<State>,
) -> Result<bool> {
    // An idle persistent connection waits for the client's next request or
    // its close; only a started request is bounded by a read timeout.
    stream.set_read_timeout(None)?;
    let mut byte = [0];
    if stream.read(&mut byte)? == 0 {
        return Ok(false);
    }
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    let mut bytes = vec![byte[0]];
    let header_end = loop {
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
        anyhow::ensure!(bytes.len() < 65536, "headers too large");
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
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
    let shared = state;
    let lock = || shared.lock().unwrap();
    {
        let mut state = lock();
        state.requests += 1;
        state.authenticated |= headers
            .to_ascii_lowercase()
            .contains("authorization: bearer grapharch-test-only");
    }
    // Query evaluation, store copies and updates run on cheap `Store` handles
    // outside the shared lock, so one slow request never queues the others.
    let transaction = url
        .path()
        .strip_prefix("/repositories/test/transactions/")
        .and_then(|x| x.parse::<u64>().ok());
    let (status, mime, body, location) = if url.path() == "/source" {
        let state = lock();
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
        let (committed, generation) = {
            let state = lock();
            (state.store.clone(), state.generation)
        };
        let store = Store::new()?;
        for q in committed.iter() {
            store.insert(q?.as_ref())?;
        }
        let mut state = lock();
        state.next += 1;
        let id = state.next;
        state
            .transactions
            .insert(id, Tx { store, generation, dirty: false });
        let location = if state.bad_location {
            format!("{base}/other/{id}")
        } else {
            format!("{base}/repositories/test/transactions/{id}")
        };
        (201, "text/plain", String::new(), Some(location))
    } else if let Some(id) = transaction {
        let store = || lock().transactions.get(&id).unwrap().store.clone();
        if method == "DELETE" {
            lock().transactions.remove(&id);
            (204, "text/plain", String::new(), None)
        } else {
            let action = params.get("action").map(String::as_str).unwrap_or("");
            match action {
                "QUERY" => {
                    let output = query_json(&store(), &body)?;
                    (
                        200,
                        "application/sparql-results+json",
                        String::from_utf8(output)?,
                        None,
                    )
                },
                "UPDATE" => {
                    store().update(&body)?;
                    lock().transactions.get_mut(&id).unwrap().dirty = true;
                    (204, "text/plain", String::new(), None)
                },
                "GET" => {
                    let mut output = String::new();
                    for q in store().iter() {
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
                    let mut state = lock();
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
                            return Ok(false);
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
        let committed = lock().store.clone();
        let output = query_json(&committed, &body)?;
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
         {}\r\nConnection: keep-alive\r\n{location}\r\n{body}",
        body.len()
    )?;
    Ok(true)
}
