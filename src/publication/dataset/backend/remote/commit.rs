//! RDF4J commit, and what a failed commit says about its outcome.
use {
    super::{health::Hazard, session::RemoteSession},
    crate::publication::sparql::{Drain, LimitedRead, http_error},
    reqwest::StatusCode,
    std::{
        io::{self, Read},
        sync::atomic::Ordering,
    },
};

#[derive(Debug)]
pub(super) struct Conflict;
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("external SPARQL transaction had a serialization conflict")
    }
}
impl std::error::Error for Conflict {}
/// What a commit request that produced no response leaves behind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum UnsentOrLost {
    /// No connection reached the service, so the commit was never sent and
    /// the transaction is known not to be committed.
    NeverSent,
    /// The request may have reached the service; whether it applied is
    /// unknown.
    Lost,
}
impl UnsentOrLost {
    pub fn of(error: &reqwest::Error) -> Self {
        if error.is_connect() {
            Self::NeverSent
        } else {
            Self::Lost
        }
    }
}
impl RemoteSession {
    pub fn commit(&self) -> io::Result<()> {
        self.flush()?;
        let mut response = match self.request("COMMIT")?.send() {
            Ok(response) => response,
            Err(error) => return Err(self.unanswered(error)),
        };
        let code = response.status();
        if code.is_success() {
            self.0.closed.store(true, Ordering::SeqCst);
            return response.drain();
        }
        let mut body = String::new();
        if let Err(error) =
            LimitedRead::new(&mut response, 8192).read_to_string(&mut body)
        {
            self.0.store.record(Hazard::CommitOutcomeUnknown);
            return Err(error);
        }
        if code == StatusCode::CONFLICT ||
            (code.is_server_error() &&
                body.contains("SailConflictException"))
        {
            return Err(io::Error::other(Conflict));
        }
        if code.is_server_error() {
            self.0.store.record(Hazard::CommitOutcomeUnknown);
        }
        Err(io::Error::other(format!(
            "external commit failed with HTTP {code}; outcome may be unknown"
        )))
    }

    /// Reports a commit that received no response, poisoning the store only
    /// when the request may have been delivered.
    fn unanswered(&self, error: reqwest::Error) -> io::Error {
        let kind = UnsentOrLost::of(&error);
        let error = http_error(error);
        match kind {
            UnsentOrLost::NeverSent => {
                io::Error::other(format!(
                    "external commit was not sent, so the transaction is not \
                     committed: {error}"
                ))
            },
            UnsentOrLost::Lost => {
                self.0.store.record(Hazard::CommitOutcomeUnknown);
                error
            },
        }
    }
}
