//! Bounded set of concurrent book worker jobs that is always drained before
//! its owner touches the staging directory again.
//!
//! Dropping a `JoinSet` aborts its tasks, and an aborted task drops its child
//! process without waiting for it to exit. The publisher then removes the
//! stage while the worker still holds its executable image and output files,
//! which Windows refuses with a sharing violation (os error 32).
use {
    super::book_worker,
    anyhow::{Result, anyhow},
    std::path::PathBuf,
    tokio::{sync::watch, task::JoinSet},
};

/// The outcome of compiling the book at one index.
pub(super) type BookJob = (usize, Result<()>);

/// Concurrent book compilations against one pinned worker image.
pub(super) struct BookPool {
    running:           JoinSet<BookJob>,
    cancel:            watch::Sender<bool>,
    executable:        PathBuf,
    root:              PathBuf,
    expected_engine:   String,
    preparation_error: Option<String>,
    capacity:          usize,
}

impl BookPool {
    /// A pool running at most `capacity` workers inside `root`.
    /// `preparation_error` fails every job when the image could not be
    /// pinned.
    pub fn new(
        executable: PathBuf,
        root: PathBuf,
        expected_engine: String,
        preparation_error: Option<String>,
        capacity: usize,
    ) -> Self {
        Self {
            running: JoinSet::new(),
            cancel: watch::channel(false).0,
            executable,
            root,
            expected_engine,
            preparation_error,
            capacity,
        }
    }

    /// Whether another job may start now.
    pub fn has_capacity(&self) -> bool { self.running.len() < self.capacity }

    /// Whether no job is running.
    pub fn is_idle(&self) -> bool { self.running.is_empty() }

    /// Start compiling `source` for the book at `index`.
    pub fn start(&mut self, index: usize, source: String) {
        let executable = self.executable.clone();
        let root = self.root.clone();
        let expected_engine = self.expected_engine.clone();
        let preparation_error = self.preparation_error.clone();
        let cancel = self.cancel.subscribe();
        self.running.spawn(async move {
            if let Some(error) = preparation_error {
                return (index, Err(anyhow!(error)));
            }
            let outcome = book_worker::run(
                &executable,
                &root,
                &source,
                &expected_engine,
                cancel,
            )
            .await;
            (index, outcome)
        });
    }

    /// Wait for the next job to finish.
    pub async fn next(&mut self) -> Result<BookJob> {
        Ok(self
            .running
            .join_next()
            .await
            .ok_or_else(|| anyhow!("book worker queue unexpectedly empty"))??)
    }

    /// Cancel every running job and wait until each worker has exited. Every
    /// owner calls this on all paths before it returns.
    pub async fn drain(mut self) -> Result<()> {
        self.cancel.send_replace(true);
        let mut failure = None;
        while let Some(joined) = self.running.join_next().await {
            if let Err(error) = joined {
                failure.get_or_insert(error);
            }
        }
        match failure {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}
