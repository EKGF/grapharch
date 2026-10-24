//! Embedded dataset with isolated contribution attempts and immutable
//! snapshots.

use {
    super::term,
    anyhow::{Result, ensure},
    oxigraph::{sparql::QueryResults, store::Store},
    oxrdf::{GraphName, Literal, NamedNode},
    sha2::{Digest, Sha256},
    std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    },
    uuid::Uuid,
};

mod attempt;
mod reader;
mod snapshot;
pub use reader::Rows;
mod transfer;
pub use attempt::Attempt;

pub(crate) const CONTROL: &str = "urn:grapharch:control";
/// Maximum serialized data per contribution chunk.
pub const MAX_CHUNK_BYTES: usize = 1024 * 1024;
/// Maximum quads per contribution chunk.
pub const MAX_CHUNK_QUADS: usize = 256;

/// Lifecycle role of a contribution; plans never become source revision inputs.
#[derive(Clone, Copy, Debug)]
pub enum Stage {
    Collect,
    Document,
    Plan,
}
impl Stage {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Collect => "collect",
            Self::Document => "document",
            Self::Plan => "plan",
        }
    }
}

/// A completed immutable contribution, independent of its current source head.
#[derive(Clone, Debug)]
pub struct CommittedContribution {
    pub(crate) owner:   NamedNode,
    pub(crate) version: NamedNode,
}

/// One shared embedded dataset. All blocking storage operations leave Tokio
/// workers.
#[derive(Clone)]
pub struct Dataset {
    pub(crate) store: Arc<Store>,
    coordination:     Arc<Mutex<()>>,
}

impl Dataset {
    /// Create an in-memory dataset.
    pub fn memory() -> Result<Self> {
        Ok(Self {
            store:        Arc::new(Store::new()?),
            coordination: Arc::new(Mutex::new(())),
        })
    }

    /// Open an exclusively owned persistent dataset without blocking the
    /// executor.
    pub async fn open(path: PathBuf) -> Result<Self> {
        Ok(Self {
            coordination: Arc::new(Mutex::new(())),
            store:        Arc::new(
                tokio::task::spawn_blocking(move || Store::open(path))
                    .await??,
            ),
        })
    }

    pub(crate) async fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Store) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let store = self.store.clone();
        let coordination = self.coordination.clone();
        tokio::task::spawn_blocking(move || {
            // Oxigraph guarantees repeatable reads, not a serializable
            // predicate guard. Serialize embedded mutations and
            // guarded promotion together.
            let _guard = coordination.lock().map_err(|_| {
                anyhow::anyhow!("dataset coordinator was poisoned")
            })?;
            operation(&store)
        })
        .await?
    }

    /// Begin a source-owned attempt. A newer attempt supersedes an older one.
    pub async fn begin(
        &self,
        project: &str,
        collector: &str,
        source: &str,
        revision: &str,
    ) -> Result<Attempt> {
        self.begin_stage(
            project,
            collector,
            source,
            revision,
            Stage::Collect,
        )
        .await
    }

    /// Register a contribution with an explicit lifecycle role.
    pub async fn begin_stage(
        &self,
        project: &str,
        collector: &str,
        source: &str,
        revision: &str,
        stage: Stage,
    ) -> Result<Attempt> {
        let project = NamedNode::new(project)?;
        let collector = NamedNode::new(collector)?;
        ensure!(
            !revision.is_empty(),
            "source revision must not be empty"
        );
        let owner = identity("contribution", &[
            project.as_str(),
            collector.as_str(),
            source,
        ]);
        let version = identity("version", &[
            owner.as_str(),
            stage.name(),
            revision,
        ]);
        let attempt = NamedNode::new(format!(
            "urn:grapharch:attempt:{}",
            Uuid::new_v4()
        ))?;
        let query = format!(
            "DELETE {{ GRAPH <{CONTROL}> {{ {owner} {} ?old . {owner} {} \
             ?failed }} }} INSERT {{ GRAPH <{CONTROL}> {{ {owner} {} \
             {attempt} . {attempt} {} {version} ; {} {collector} ; {} \
             {project} ; <https://ekgf.org/ontology/grapharch#stage> {} ; {} {} ; {} {} }} }} WHERE {{ OPTIONAL {{ GRAPH \
             <{CONTROL}> {{ {owner} {} ?old }} }} OPTIONAL {{ GRAPH \
             <{CONTROL}> {{ {owner} {} ?failed }} }} }}",
            term("pending"),
            term("failed"),
            term("pending"),
            term("version"),
            term("collector"),
            term("project"),
            Literal::new_simple_literal(stage.name()),
            term("source"),
            Literal::new_simple_literal(source),
            term("revision"),
            Literal::new_simple_literal(revision),
            term("pending"),
            term("failed")
        );
        self.run(move |store| {
            store.update(query.as_str())?;
            Ok(())
        })
        .await?;
        Ok(Attempt {
            dataset: self.clone(),
            owner,
            version,
            id: attempt,
            stage,
        })
    }

    /// Remove one registered source contribution without deleting historical
    /// revisions.
    pub async fn remove(
        &self,
        project: &str,
        processor: &str,
        source: &str,
    ) -> Result<()> {
        let owner = identity("contribution", &[project, processor, source]);
        self.run(move |store| {
            let query = format!(
                "DELETE {{ GRAPH <{CONTROL}> {{ {owner} ?state ?value }} }} \
                 WHERE {{ GRAPH <{CONTROL}> {{ {owner} ?state ?value VALUES \
                 ?state {{ {} {} {} }} }} }}",
                term("head"),
                term("pending"),
                term("failed")
            );
            store.update(query.as_str())?;
            Ok(())
        })
        .await
    }

    /// Determine whether a source's current complete version already matches
    /// its dependencies.
    pub async fn is_current(
        &self,
        stage: Stage,
        project: &str,
        processor: &str,
        source: &str,
        revision: &str,
    ) -> Result<bool> {
        let owner = identity("contribution", &[project, processor, source]);
        let version = identity("version", &[
            owner.as_str(),
            stage.name(),
            revision,
        ]);
        self.run(move |store| {
            let query = format!(
                "ASK {{ GRAPH <{CONTROL}> {{ {owner} {} {version} FILTER NOT \
                 EXISTS {{ {owner} {} ?pending }} FILTER NOT EXISTS {{ \
                 {owner} {} ?failed }} }} }}",
                term("head"),
                term("pending"),
                term("failed")
            );
            match store.query(query.as_str())? {
                QueryResults::Boolean(value) => Ok(value),
                _ => anyhow::bail!("expected current-version result"),
            }
        })
        .await
    }

    /// Reject assembly while the project's latest source attempts are
    /// incomplete or failed.
    pub async fn ensure_ready(&self, project: &str) -> Result<()> {
        let project = NamedNode::new(project)?;
        self.run(move |store| {
            let query = format!(
                "SELECT ?source ?state WHERE {{ GRAPH <{CONTROL}> {{ ?owner \
                 ?state ?attempt . ?attempt {} {project} ; {} ?source . \
                 VALUES ?state {{ {} {} }} }} }} LIMIT 1",
                term("project"),
                term("source"),
                term("pending"),
                term("failed")
            );
            let QueryResults::Solutions(mut rows) =
                store.query(query.as_str())?
            else {
                anyhow::bail!("expected source readiness result")
            };
            if let Some(row) = rows.next().transpose()? {
                anyhow::bail!(
                    "project has an incomplete or failed input: {} ({})",
                    row["source"],
                    row["state"]
                );
            }
            Ok(())
        })
        .await
    }
}

pub(crate) fn identity(kind: &str, parts: &[&str]) -> NamedNode {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part.len().to_be_bytes());
        digest.update(part.as_bytes());
    }
    NamedNode::new_unchecked(format!(
        "urn:grapharch:{kind}:{:x}",
        digest.finalize()
    ))
}

pub(crate) fn original_graph(graph: &GraphName) -> Literal {
    // N-Quads graph spelling distinguishes default, named, and blank graphs
    // without reserving a user-visible IRI that could alias the default graph.
    Literal::new_simple_literal(graph.to_string())
}

#[cfg(test)]
mod coordination_tests {
    use {
        super::*,
        std::{sync::Barrier, time::Duration},
    };

    #[tokio::test]
    async fn guarded_metadata_checks_and_source_writes_share_one_coordination_boundary()
    -> Result<()> {
        let dataset = Dataset::memory()?;
        let gate = Arc::new(Barrier::new(2));
        let held_gate = gate.clone();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let guarded_dataset = dataset.clone();
        let held = tokio::spawn(async move {
            guarded_dataset
                .run(move |_store| {
                    let _ = entered.send(());
                    held_gate.wait();
                    Ok(())
                })
                .await
        });
        ready.await?;
        let mut writer = tokio::spawn(async move {
            dataset
                .begin("urn:project", "urn:collector", "source", "r1")
                .await
        });
        let outcome =
            tokio::time::timeout(Duration::from_millis(30), &mut writer).await;
        let blocked = outcome.is_err();
        tokio::task::spawn_blocking(move || gate.wait()).await?;
        held.await??;
        if blocked {
            writer.await??;
        } else {
            outcome???;
        }
        ensure!(
            blocked,
            "a source write overtook the guarded metadata operation"
        );
        Ok(())
    }
}
