//! Removal of files that another process has only just let go of.
//!
//! On Windows a file stays locked for a short while after the last process
//! using it exits: the loader keeps an executable's image mapped, and
//! antivirus and indexing services open freshly written files. Deleting a book
//! worker executable right after its process was waited for, or a stage that
//! one wrote into, then fails with a sharing violation although nothing in
//! GraphArch still holds it. Such a removal is retried for a bounded time;
//! every other error, and every error on other platforms, is returned at once.

use {
    std::{io, path::Path, time::Duration},
    tokio::time::sleep,
};

/// How long a removal waits for a just-released file to become deletable.
#[derive(Clone, Copy)]
pub(super) struct FileRelease {
    attempts:  u32,
    first:     Duration,
    transient: fn(&io::Error) -> bool,
}

impl FileRelease {
    /// Up to about three seconds in all, doubling from 25 ms.
    pub const fn platform() -> Self {
        Self {
            attempts:  8,
            first:     Duration::from_millis(25),
            transient: is_sharing_violation,
        }
    }

    pub async fn remove_file(self, path: &Path) -> io::Result<()> {
        self.retry(|| tokio::fs::remove_file(path)).await
    }

    pub async fn remove_dir_all(self, path: &Path) -> io::Result<()> {
        self.retry(|| tokio::fs::remove_dir_all(path)).await
    }

    async fn retry<F, Fut>(self, mut operation: F) -> io::Result<()>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = io::Result<()>>,
    {
        let mut delay = self.first;
        let mut attempt = 1;
        loop {
            match operation().await {
                Err(error)
                    if attempt < self.attempts && (self.transient)(&error) =>
                {
                    sleep(delay).await;
                    delay = delay.saturating_mul(2);
                    attempt += 1;
                },
                outcome => return outcome,
            }
        }
    }
}

/// `ERROR_ACCESS_DENIED` (deleting an image that is still mapped) and
/// `ERROR_SHARING_VIOLATION`.
#[cfg(windows)]
fn is_sharing_violation(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32))
}

#[cfg(not(windows))]
fn is_sharing_violation(_: &io::Error) -> bool { false }

#[cfg(test)]
mod tests {
    use {super::*, std::cell::Cell};

    fn busy() -> io::Error { io::Error::other("busy") }

    fn quick(attempts: u32) -> FileRelease {
        FileRelease {
            attempts,
            first: Duration::from_millis(1),
            transient: |error| error.to_string() == "busy",
        }
    }

    #[tokio::test]
    async fn a_released_file_is_removed_on_a_later_attempt() -> io::Result<()> {
        let calls = Cell::new(0);
        quick(4)
            .retry(|| {
                calls.set(calls.get() + 1);
                let outcome =
                    if calls.get() < 3 { Err(busy()) } else { Ok(()) };
                async move { outcome }
            })
            .await?;
        assert_eq!(calls.get(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn a_file_that_stays_locked_fails_after_the_last_attempt() {
        let calls = Cell::new(0);
        let outcome = quick(4)
            .retry(|| {
                calls.set(calls.get() + 1);
                async { Err(busy()) }
            })
            .await;
        assert!(outcome.is_err());
        assert_eq!(calls.get(), 4);
    }

    #[tokio::test]
    async fn any_other_error_is_returned_at_once() {
        let calls = Cell::new(0);
        let outcome = quick(4)
            .retry(|| {
                calls.set(calls.get() + 1);
                async { Err(io::Error::from(io::ErrorKind::NotFound)) }
            })
            .await;
        assert_eq!(
            outcome.err().map(|error| error.kind()),
            Some(io::ErrorKind::NotFound)
        );
        assert_eq!(calls.get(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn sharing_violations_are_transient_on_windows() {
        assert!(is_sharing_violation(
            &io::Error::from_raw_os_error(32)
        ));
        assert!(is_sharing_violation(
            &io::Error::from_raw_os_error(5)
        ));
        assert!(!is_sharing_violation(
            &io::Error::from_raw_os_error(2)
        ));
    }
}
