use {
    super::{CONTROL, Dataset, original_graph},
    crate::publication::term,
    anyhow::{Result, ensure},
    oxigraph::sparql::{QueryResults, QuerySolution},
    oxrdf::{NamedNode, Quad, Term},
    tokio::{sync::mpsc, task::JoinHandle},
};

/// Bounded streaming query results. Dropping the receiver releases a blocked
/// producer.
pub struct Rows {
    receiver: mpsc::Receiver<QuerySolution>,
    task:     Option<JoinHandle<Result<()>>>,
}

impl Rows {
    /// Read the next row, propagating parser, evaluator, and producer errors at
    /// EOF.
    pub async fn next(&mut self) -> Result<Option<QuerySolution>> {
        if let Some(row) = self.receiver.recv().await {
            return Ok(Some(row));
        }
        if let Some(task) = self.task.take() {
            task.await??;
        }
        Ok(None)
    }
}

impl Dataset {
    /// A logical triple pattern scoped to a pinned snapshot, across
    /// contributions. Use distinct discriminators for joins. Pattern text
    /// is SPARQL authored by the caller; source-native text must be
    /// represented as escaped RDF terms.
    pub fn logical_pattern(
        snapshot: &NamedNode,
        pattern: &str,
        discriminator: u16,
    ) -> String {
        format!(
            "GRAPH {snapshot} {{ ?_owner{discriminator} {} \
             ?_version{discriminator} }} GRAPH <{CONTROL}> {{ \
             ?_version{discriminator} {} ?_attempt{discriminator} . \
             ?_physical{discriminator} {} ?_attempt{discriminator} ; {} \
             ?_original{discriminator} }} GRAPH ?_physical{discriminator} {{ \
             {pattern} }}",
            term("head"),
            term("data"),
            term("attempt"),
            term("original")
        )
    }

    /// Execute an explicit SPARQL SELECT through a bounded result channel.
    /// Control graphs are visible to inspection queries; use logical_pattern
    /// for publication data so staging and superseded contributions cannot
    /// leak in.
    pub async fn select(&self, query: String) -> Result<Rows> {
        #[cfg(test)]
        self.select_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ensure!(
            query.len() <= 64 * 1024,
            "query exceeds byte limit"
        );
        tracing::trace!(%query, "executing SPARQL SELECT");
        let store = self.store.clone();
        let (sender, receiver) = mpsc::channel(2);
        let task = tokio::task::spawn_blocking(move || {
            let QueryResults::Solutions(rows) = store.query(query.as_str())?
            else {
                anyhow::bail!("expected SELECT query")
            };
            for row in rows {
                let row = row?;
                ensure!(
                    row.len() <= 32,
                    "query exceeds projected-column limit"
                );
                if sender.blocking_send(row).is_err() {
                    return Ok(());
                }
            }
            Ok(())
        });
        Ok(Rows { receiver, task: Some(task) })
    }
}

impl Dataset {
    /// Query a pinned snapshot for a logical quad, preserving source graph
    /// identity.
    pub async fn contains(
        &self,
        snapshot: &NamedNode,
        quad: Quad,
    ) -> Result<bool> {
        let snapshot = snapshot.clone();
        self.run(move |store| {
            let original = original_graph(&quad.graph_name);
            let query = format!(
                "ASK {{ GRAPH {snapshot} {{ ?owner {} ?version }} GRAPH \
                 <{CONTROL}> {{ ?version {} ?attempt . ?physical {} ?attempt \
                 ; {} {original} }} GRAPH ?physical {{ {} {} {} }} }}",
                term("head"),
                term("data"),
                term("attempt"),
                term("original"),
                quad.subject,
                quad.predicate,
                quad.object
            );
            match store.query(query.as_str())? {
                QueryResults::Boolean(value) => Ok(value),
                _ => anyhow::bail!("expected ASK result"),
            }
        })
        .await
    }

    /// Number of distinct active assertions, counting independent
    /// contributions.
    pub async fn active_count(&self) -> Result<u64> {
        self.run(|store| {
            let query = format!(
                "SELECT (COUNT(*) AS ?count) WHERE {{ GRAPH <{CONTROL}> {{ \
                 ?owner {} ?version . ?version {} ?attempt . ?physical {} \
                 ?attempt }} GRAPH ?physical {{ ?s ?p ?o }} }}",
                term("head"),
                term("data"),
                term("attempt")
            );
            let QueryResults::Solutions(mut rows) =
                store.query(query.as_str())?
            else {
                anyhow::bail!("expected count")
            };
            let row = rows
                .next()
                .transpose()?
                .ok_or_else(|| anyhow::anyhow!("missing count"))?;
            let Some(Term::Literal(count)) = row.get("count") else {
                anyhow::bail!("invalid count")
            };
            Ok(count.value().parse()?)
        })
        .await
    }
}
