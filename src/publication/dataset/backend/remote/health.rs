//! Whether a store may still be used after a request with an unknown effect.
use std::{
    io,
    sync::atomic::{AtomicU8, Ordering},
};

/// Why a store refuses further operations, if it does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(super) enum Hazard {
    /// Nothing is in doubt.
    None                 = 0,
    /// A commit was sent and no response settled whether it applied.
    CommitOutcomeUnknown = 1,
    /// An abandoned transaction could not be rolled back and may still hold
    /// the service's resources.
    AbandonedTransaction = 2,
}
impl Hazard {
    fn from_repr(value: u8) -> Self {
        match value {
            1 => Self::CommitOutcomeUnknown,
            2 => Self::AbandonedTransaction,
            _ => Self::None,
        }
    }

    fn error(self) -> Option<io::Error> {
        let message = match self {
            Self::None => return None,
            Self::CommitOutcomeUnknown => {
                "external commit outcome is unknown; reopen and inspect the \
                 dataset before further operations"
            },
            Self::AbandonedTransaction => {
                "an abandoned external transaction could not be rolled back; \
                 reopen the dataset before further operations"
            },
        };
        Some(io::Error::other(message))
    }
}
/// The first hazard recorded for a store; later ones never replace it.
pub(super) struct Health(AtomicU8);
impl Health {
    pub fn new() -> Self { Self(AtomicU8::new(Hazard::None as u8)) }

    pub fn record(&self, hazard: Hazard) {
        // A store that already refuses work keeps its first reason.
        let _ = self.0.compare_exchange(
            Hazard::None as u8,
            hazard as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    pub fn check(&self) -> io::Result<()> {
        match Hazard::from_repr(self.0.load(Ordering::SeqCst)).error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
