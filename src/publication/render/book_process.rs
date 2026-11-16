//! One native book worker child process that is always reaped before its
//! owner returns.
//!
//! A child that is only dropped (`kill_on_drop`) is signalled but not waited
//! for. On Windows, its open files and its executable image in the staging
//! directory stay in use until the process has terminated, so removing the
//! stage straight afterwards fails with a sharing violation (os error 32).
use {
    anyhow::{Result, anyhow, bail, ensure},
    std::{path::Path, process::ExitStatus, time::Duration},
    tokio::{
        io::AsyncReadExt,
        process::{Child, ChildStderr, Command},
    },
};

/// Longest time one worker may run before it is killed.
const WORKER_DEADLINE: Duration = Duration::from_secs(1800);

/// Upper bound on the diagnostics a worker may write to its error pipe.
const DIAGNOSTIC_BYTES: u64 = 64 * 1024;

/// A running `grapharch publication <subcommand>` child.
pub(super) struct BookProcess {
    child:  Child,
    stderr: Option<ChildStderr>,
}

impl BookProcess {
    /// Start `executable publication <arguments>` with a parent-watch pipe.
    pub fn spawn(
        executable: &Path,
        arguments: &[&std::ffi::OsStr],
    ) -> Result<Self> {
        let mut command = Command::new(executable);
        command
            .arg("publication")
            .args(arguments)
            .arg("--watch-parent")
            .env("TOKIO_WORKER_THREADS", "1");
        Self::start(command)
    }

    /// Start `command` with the worker's pipes and lifetime policy.
    fn start(mut command: Command) -> Result<Self> {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stderr = child.stderr.take();
        ensure!(stderr.is_some(), "missing book worker error pipe");
        Ok(Self { child, stderr })
    }

    /// Collect the worker's diagnostics and exit status. Every path that
    /// does not observe a normal exit kills the process and waits for it, so
    /// no handle of the worker outlives this call.
    pub async fn finish(
        &mut self,
        mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<Completion> {
        let outcome = self.observe(cancel.as_mut()).await;
        if outcome.is_err() {
            self.reap().await?;
        }
        outcome
    }

    async fn observe(
        &mut self,
        cancel: Option<&mut tokio::sync::watch::Receiver<bool>>,
    ) -> Result<Completion> {
        let stderr = self
            .stderr
            .take()
            .ok_or_else(|| anyhow!("book worker error pipe already taken"))?;
        let cancelled = async {
            match cancel {
                Some(cancel) => {
                    // A closed channel means the coordinator is gone.
                    let _ = cancel.changed().await;
                },
                None => std::future::pending::<()>().await,
            }
        };
        let diagnostics = tokio::select! {
            result = WorkerDiagnostics::read(stderr) => result?,
            () = cancelled => bail!("book compilation cancelled"),
            () = tokio::time::sleep(WORKER_DEADLINE) => {
                bail!("book worker exceeded the 30-minute deadline")
            },
        };
        let status = self.child.wait().await?;
        Ok(Completion { status, diagnostics })
    }

    /// Kill the process if it is still running and wait until it has exited.
    async fn reap(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            // `Child::kill` sends the signal and then waits for termination.
            self.child.kill().await?;
        }
        Ok(())
    }
}

/// The exit status and diagnostics of a worker that ran to its own end.
pub(super) struct Completion {
    status:      ExitStatus,
    diagnostics: WorkerDiagnostics,
}

impl Completion {
    /// Fail unless the worker exited successfully, naming `task`.
    pub fn succeeded(self, task: &str) -> Result<()> {
        ensure!(
            self.status.success(),
            "{task} exited with {}: {}",
            self.status,
            self.diagnostics
        );
        Ok(())
    }
}

/// Bounded diagnostics read from a worker's error pipe.
pub(super) struct WorkerDiagnostics(Vec<u8>);

impl WorkerDiagnostics {
    /// Read the pipe to its end, refusing more than the diagnostic budget.
    pub async fn read(stderr: ChildStderr) -> Result<Self> {
        // bounded: worker diagnostics are capped independently of output size.
        let mut message = Vec::new();
        stderr
            .take(DIAGNOSTIC_BYTES + 1)
            .read_to_end(&mut message)
            .await?;
        ensure!(
            message.len() as u64 <= DIAGNOSTIC_BYTES,
            "book worker exceeded diagnostic byte budget"
        );
        Ok(Self(message))
    }
}

impl std::fmt::Display for WorkerDiagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAND_IN: &str = "GRAPHARCH_BOOK_PROCESS_STAND_IN";
    const STAND_IN_TEST: &str =
        "publication::render::book_process::tests::worker_stand_in";

    /// Not a test of its own: the child process of
    /// `a_cancelled_worker_has_released_its_files_when_finish_returns`. It
    /// holds a file inside the stage open until it is killed, like a book
    /// worker writing its PDF.
    #[test]
    #[ignore = "runs only as a child of another test"]
    fn worker_stand_in() -> Result<()> {
        let Some(path) = std::env::var_os(STAND_IN) else {
            return Ok(());
        };
        let _held = std::fs::File::create(path)?;
        eprintln!("holding");
        let mut parent = String::new();
        std::io::stdin().read_line(&mut parent)?;
        Ok(())
    }

    #[tokio::test]
    async fn a_cancelled_worker_has_released_its_files_when_finish_returns()
    -> Result<()> {
        let stage = tempfile::tempdir()?;
        let held = stage.path().join("book.pdf");
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["--ignored", "--exact", STAND_IN_TEST, "--nocapture"])
            .env(STAND_IN, &held);
        let mut process = BookProcess::start(command)?;
        let (cancel, cancelled) = tokio::sync::watch::channel(false);
        let started = async {
            while !held.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            cancel.send_replace(true);
        };
        let (outcome, ()) =
            tokio::join!(process.finish(Some(cancelled)), started);
        let error = outcome
            .err()
            .ok_or_else(|| anyhow!("worker was not cancelled"))?;
        assert_eq!(error.to_string(), "book compilation cancelled");
        assert!(
            process.child.try_wait()?.is_some(),
            "finish returned while the worker was still running"
        );
        // On Windows this fails with os error 32 if the worker still runs.
        std::fs::remove_dir_all(stage.path())?;
        Ok(())
    }
}
