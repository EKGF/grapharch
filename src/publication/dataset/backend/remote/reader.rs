//! Streamed response bodies that keep their transaction open, and the
//! rollback of transactions abandoned without commit or rollback.
use {
    super::{
        health::Hazard,
        session::{RemoteSession, SessionInner},
    },
    crate::publication::sparql::{Drain, LimitedRead, Settle, http_error},
    reqwest::blocking::Response,
    std::{
        io::{self, Read},
        sync::atomic::Ordering,
    },
};

/// A result body read within its transaction.
///
/// Dropping it reads what the parser left unread, so the connection returns
/// to the store's pool.
pub(super) struct SessionReader {
    response: LimitedRead<Response>,
    _session: RemoteSession,
}
impl SessionReader {
    /// Accepts a successful response of the `expected` media type.
    pub fn expecting(
        response: Response,
        expected: &str,
        refusal: &'static str,
        session: &RemoteSession,
    ) -> io::Result<Self> {
        if !response.status().is_success() {
            return Err(response.settle().err().unwrap_or_else(|| {
                io::Error::other("SPARQL service returned an error status")
            }));
        }
        let mime = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !mime.split(';').next().is_some_and(|v| v.trim() == expected) {
            return Err(io::Error::other(refusal));
        }
        Ok(Self {
            response: LimitedRead::new(
                response,
                session.0.store.config.http.max_response_bytes,
            ),
            _session: session.clone(),
        })
    }
}
impl Read for SessionReader {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.response.read(b)
    }
}
impl Drop for SessionReader {
    fn drop(&mut self) {
        // A body that cannot be drained only costs its pooled connection.
        let _ = self.response.drain();
    }
}
impl Drop for SessionInner {
    fn drop(&mut self) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let outcome = self
            .store
            .client
            .delete(self.url.clone())
            .send()
            .map_err(http_error)
            .and_then(Settle::settle);
        if let Err(error) = outcome {
            tracing::error!(
                %error,
                "could not roll back abandoned external transaction"
            );
            self.store.record(Hazard::AbandonedTransaction);
        }
    }
}
