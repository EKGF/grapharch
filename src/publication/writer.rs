//! Bounded central write coordinator for concurrent contribution producers.

mod stream;

use {
    super::dataset::{Attempt, Dataset, MAX_CHUNK_BYTES, Stage},
    anyhow::{Result, ensure},
    oxrdf::Quad,
    std::sync::Arc,
    tokio::{
        sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
        task::JoinHandle,
    },
};

/// A cloneable entry point shared by specialized collectors.
#[derive(Clone)]
pub struct GraphWriter {
    sender:     mpsc::Sender<Command>,
    bytes:      Arc<Semaphore>,
    byte_limit: usize,
}

enum Command {
    Diagnostic {
        project:    String,
        processor:  String,
        source:     String,
        stage:      String,
        diagnostic: super::diagnostics::Diagnostic,
        _permit:    OwnedSemaphorePermit,
        reply:      oneshot::Sender<Result<()>>,
    },
    Refine {
        attempt:  Attempt,
        revision: String,
        _permit:  OwnedSemaphorePermit,
        reply:    oneshot::Sender<Result<Attempt>>,
    },
    Remove {
        project:   String,
        processor: String,
        source:    String,
        _permit:   OwnedSemaphorePermit,
        reply:     oneshot::Sender<Result<()>>,
    },
    Begin {
        stage:     Stage,
        _permit:   OwnedSemaphorePermit,
        project:   String,
        collector: String,
        source:    String,
        revision:  String,
        reply:     oneshot::Sender<Result<Attempt>>,
    },
    Chunk {
        attempt: Attempt,
        quads:   Vec<Quad>,
        _permit: OwnedSemaphorePermit,
        reply:   oneshot::Sender<Result<()>>,
    },
    Complete {
        attempt: Attempt,
        reply:   oneshot::Sender<Result<()>>,
    },
    Abort {
        attempt: Attempt,
        reply:   oneshot::Sender<Result<()>>,
    },
}

/// A single producer's ordered stream. Completion consumes it, after
/// acknowledged chunks.
pub struct ContributionStream {
    writer:         GraphWriter,
    attempt:        Attempt,
    failed:         bool,
    // bounded: at most MAX_CHUNK_QUADS and MAX_CHUNK_BYTES before flushing.
    pending:        Vec<Quad>,
    pending_bytes:  usize,
    needs_revision: bool,
}

impl GraphWriter {
    /// Start the embedded backend's serial writer. Both queue count and bytes
    /// are bounded.
    pub fn spawn(
        dataset: Dataset,
        capacity: usize,
        byte_limit: usize,
    ) -> Result<(Self, JoinHandle<Result<()>>)> {
        ensure!(
            capacity > 0,
            "writer queue capacity must be positive"
        );
        ensure!(
            byte_limit > 0 && byte_limit <= u32::MAX as usize,
            "writer byte limit must fit a positive u32"
        );
        let (sender, mut receiver) = mpsc::channel(capacity);
        let bytes = Arc::new(Semaphore::new(byte_limit));
        let handle = tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    Command::Diagnostic {
                        project,
                        processor,
                        source,
                        stage,
                        diagnostic,
                        _permit,
                        reply,
                    } => {
                        let result = super::diagnostics::record(
                            &dataset, &project, &processor, &source, &stage,
                            diagnostic,
                        )
                        .await;
                        let _ = reply.send(result);
                    },
                    Command::Refine { attempt, revision, _permit, reply } => {
                        let _ =
                            reply.send(attempt.with_revision(&revision).await);
                    },
                    Command::Remove {
                        project,
                        processor,
                        source,
                        _permit,
                        reply,
                    } => {
                        let _ = reply.send(
                            dataset.remove(&project, &processor, &source).await,
                        );
                    },
                    Command::Begin {
                        stage,
                        _permit,
                        project,
                        collector,
                        source,
                        revision,
                        reply,
                    } => {
                        let result = dataset
                            .begin_stage(
                                &project, &collector, &source, &revision, stage,
                            )
                            .await;
                        let _ = reply.send(result);
                    },
                    Command::Chunk { attempt, quads, _permit, reply } => {
                        let result = attempt.push(quads).await;
                        // Storage errors invalidate this stream, not unrelated
                        // sources.
                        let _ = reply.send(result);
                    },
                    Command::Complete { attempt, reply } => {
                        let _ = reply.send(attempt.complete().await);
                    },
                    Command::Abort { attempt, reply } => {
                        let _ = reply.send(attempt.abort().await);
                    },
                }
            }
            Ok(())
        });
        Ok((Self { sender, bytes, byte_limit }, handle))
    }

    /// Persist an error as an output diagnostic and documentation fragment.
    /// Diagnostic records never become source contribution heads.
    pub async fn record_diagnostic(
        &self,
        project: &str,
        processor: &str,
        source: &str,
        stage: &str,
        diagnostic: super::diagnostics::Diagnostic,
    ) -> Result<()> {
        let size = (project.len() +
            processor.len() +
            source.len() +
            stage.len() +
            diagnostic.message.len() +
            diagnostic.reference.len() +
            diagnostic.code.len() +
            diagnostic.source_fragment.as_ref().map_or(0, String::len)) *
            4 +
            2048;
        ensure!(
            size <= MAX_CHUNK_BYTES && size <= self.byte_limit,
            "diagnostic exceeds writer byte budget"
        );
        let permit = self.bytes.clone().acquire_many_owned(size as u32).await?;
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Diagnostic {
                project: project.into(),
                processor: processor.into(),
                source: source.into(),
                stage: stage.into(),
                diagnostic,
                _permit: permit,
                reply,
            })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        response.await.map_err(|_| {
            anyhow::anyhow!("graph writer dropped diagnostic reply")
        })?
    }

    /// Remove one owned source after successful enumeration or an explicit
    /// removal request.
    pub async fn remove(
        &self,
        project: &str,
        processor: &str,
        source: &str,
    ) -> Result<()> {
        let size = project.len() + processor.len() + source.len() + 256;
        ensure!(
            size <= MAX_CHUNK_BYTES && size <= self.byte_limit,
            "removal metadata exceeds byte limit"
        );
        let permit = self.bytes.clone().acquire_many_owned(size as u32).await?;
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Remove {
                project: project.to_owned(),
                processor: processor.to_owned(),
                source: source.to_owned(),
                _permit: permit,
                reply,
            })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        response.await.map_err(|_| {
            anyhow::anyhow!(
                "graph writer stopped before removal acknowledgement"
            )
        })?
    }

    /// Register an attempt before reading or parsing its input. Its final
    /// source revision must be set explicitly before completion.
    pub async fn begin_pending(
        &self,
        project: &str,
        processor: &str,
        source: &str,
    ) -> Result<ContributionStream> {
        let pending = format!("pending:{}", uuid::Uuid::new_v4());
        let mut stream =
            self.begin(project, processor, source, &pending).await?;
        stream.needs_revision = true;
        Ok(stream)
    }

    /// Begin a stream through the central coordinator.
    pub async fn begin(
        &self,
        project: &str,
        collector: &str,
        source: &str,
        revision: &str,
    ) -> Result<ContributionStream> {
        self.begin_stage(
            project,
            collector,
            source,
            revision,
            Stage::Collect,
        )
        .await
    }

    /// Begin a source, document, or plan contribution through the same
    /// coordinator.
    pub async fn begin_stage(
        &self,
        project: &str,
        collector: &str,
        source: &str,
        revision: &str,
        stage: Stage,
    ) -> Result<ContributionStream> {
        let size = project.len() +
            collector.len() +
            source.len() +
            revision.len() +
            256;
        ensure!(
            size <= MAX_CHUNK_BYTES && size <= self.byte_limit,
            "contribution metadata exceeds byte limit"
        );
        let permit = self.bytes.clone().acquire_many_owned(size as u32).await?;
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Begin {
                stage,
                _permit: permit,
                project: project.to_owned(),
                collector: collector.to_owned(),
                source: source.to_owned(),
                revision: revision.to_owned(),
                reply,
            })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        let attempt = response.await.map_err(|_| {
            anyhow::anyhow!("graph writer stopped before begin acknowledgement")
        })??;
        Ok(ContributionStream {
            writer: self.clone(),
            attempt,
            failed: false,
            pending: Vec::new(),
            pending_bytes: 0,
            needs_revision: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        oxrdf::{GraphName, Literal, NamedNode},
        std::time::Duration,
    };

    #[tokio::test]
    async fn slow_consumer_applies_byte_backpressure_and_cancellation_releases_capacity()
    -> Result<()> {
        let dataset = Dataset::memory()?;
        let attempt = dataset
            .begin("urn:project", "urn:collector", "source", "r1")
            .await?;
        let quad = Quad::new(
            NamedNode::new("urn:s")?,
            NamedNode::new("urn:p")?,
            Literal::new_simple_literal("body"),
            GraphName::DefaultGraph,
        );
        let size = quad.to_string().len() + 256;
        let (sender, mut receiver) = mpsc::channel(1);
        let writer = GraphWriter {
            sender,
            bytes: Arc::new(Semaphore::new(size)),
            byte_limit: size,
        };
        let mut first = ContributionStream {
            writer:         writer.clone(),
            attempt:        attempt.clone(),
            failed:         false,
            pending:        Vec::new(),
            pending_bytes:  0,
            needs_revision: false,
        };
        let first_quad = quad.clone();
        let first_task =
            tokio::spawn(async move { first.push(vec![first_quad]).await });
        // Hold the dequeued command: its permit covers in-flight work too.
        let held = receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("missing chunk"))?;
        assert_eq!(writer.bytes.available_permits(), 0);
        let mut second = ContributionStream {
            writer: writer.clone(),
            attempt,
            failed: false,
            pending: Vec::new(),
            pending_bytes: 0,
            needs_revision: false,
        };
        let mut blocked = Box::pin(second.push(vec![quad]));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut blocked)
                .await
                .is_err()
        );
        drop(blocked);
        drop(held);
        drop(receiver);
        assert!(first_task.await?.is_err());
        assert_eq!(writer.bytes.available_permits(), size);
        Ok(())
    }
}
