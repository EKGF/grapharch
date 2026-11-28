//! How a commit request without a response is classified.
use {
    super::commit::UnsentOrLost,
    std::{
        io::Read,
        net::{SocketAddr, TcpListener},
        thread,
    },
};

fn commit_to(address: SocketAddr) -> reqwest::Error {
    reqwest::blocking::Client::new()
        .put(format!(
            "http://{address}/repositories/test/transactions/1"
        ))
        .query(&[("action", "COMMIT")])
        .send()
        .expect_err("the commit must receive no response")
}
#[test]
fn a_refused_connection_means_the_commit_was_never_sent() {
    let address = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("reserve a loopback port");
    // The listener is closed, so every connection to it is refused.
    assert_eq!(
        UnsentOrLost::of(&commit_to(address)),
        UnsentOrLost::NeverSent
    );
}
#[test]
fn a_connection_closed_after_the_request_leaves_the_outcome_unknown() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept the commit");
        let mut request = [0; 1024];
        // The request arrived; closing without a response loses its outcome.
        let read = stream.read(&mut request).expect("read the commit");
        assert!(
            read > 0,
            "the commit request must reach the service"
        );
    });
    assert_eq!(
        UnsentOrLost::of(&commit_to(address)),
        UnsentOrLost::Lost
    );
    server.join().expect("mock service");
}
