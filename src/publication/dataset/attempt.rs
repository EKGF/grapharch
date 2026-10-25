use {
    super::{
        CONTROL,
        Dataset,
        MAX_CHUNK_BYTES,
        MAX_CHUNK_QUADS,
        backend::Transaction,
        identity,
        original_graph,
    },
    crate::publication::term,
    anyhow::{Result, ensure},
    oxigraph::sparql::QueryResults,
    oxrdf::{NamedNode, Quad},
    std::io,
};

/// One isolated, source-owned streaming contribution attempt.
/// Dropping it never promotes incomplete assertions.
#[derive(Clone)]
pub struct Attempt {
    pub(super) dataset: Dataset,
    pub(super) owner:   NamedNode,
    pub(super) version: NamedNode,
    pub(super) id:      NamedNode,
    pub(super) stage:   super::Stage,
}

impl Attempt {
    pub(crate) fn receipt(&self) -> super::CommittedContribution {
        super::CommittedContribution {
            owner:   self.owner.clone(),
            version: self.version.clone(),
        }
    }

    /// Stable source contribution identity.
    pub fn owner(&self) -> &NamedNode { &self.owner }

    /// Write a bounded chunk to private staging graphs.
    pub async fn push(&self, quads: Vec<Quad>) -> Result<()> {
        ensure!(
            quads.len() <= MAX_CHUNK_QUADS,
            "chunk exceeds quad limit"
        );
        let mut bytes = 0usize;
        for quad in &quads {
            bytes = bytes
                .checked_add(quad.to_string().len() + 256)
                .ok_or_else(|| anyhow::anyhow!("chunk size overflow"))?;
            ensure!(
                bytes <= MAX_CHUNK_BYTES,
                "chunk exceeds byte limit"
            );
        }
        let attempt = self.clone();
        self.dataset
            .run(move |store| {
                store.transaction::<_, io::Error>(|mut transaction| {
                    attempt.check_pending(&transaction)?;
                    for quad in &quads {
                        let original = original_graph(&quad.graph_name);
                        let physical = identity("graph", &[
                            attempt.id.as_str(),
                            original.value(),
                        ]);
                        transaction.insert(
                            Quad::new(
                                physical.clone(),
                                term("attempt"),
                                attempt.id.clone(),
                                NamedNode::new_unchecked(CONTROL),
                            )
                            .as_ref(),
                        )?;
                        transaction.insert(
                            Quad::new(
                                physical.clone(),
                                term("original"),
                                original,
                                NamedNode::new_unchecked(CONTROL),
                            )
                            .as_ref(),
                        )?;
                        transaction.insert(
                            Quad::new(
                                quad.subject.clone(),
                                quad.predicate.clone(),
                                quad.object.clone(),
                                physical,
                            )
                            .as_ref(),
                        )?;
                    }
                    Ok(())
                })?;
                Ok(())
            })
            .await
    }

    fn check_pending(&self, transaction: &Transaction<'_>) -> io::Result<()> {
        if !transaction.contains(
            Quad::new(
                self.owner.clone(),
                term("pending"),
                self.id.clone(),
                NamedNode::new_unchecked(CONTROL),
            )
            .as_ref(),
        )? {
            return Err(io::Error::other(
                "contribution attempt was superseded or already completed",
            ));
        }
        Ok(())
    }

    pub(crate) async fn with_revision(
        mut self,
        revision: &str,
    ) -> Result<Self> {
        ensure!(
            !revision.is_empty(),
            "source revision must not be empty"
        );
        let next_version = identity("version", &[
            self.owner.as_str(),
            self.stage.name(),
            revision,
        ]);
        let attempt = self.clone();
        let revision = revision.to_owned();
        let version = next_version.clone();
        self.dataset
            .run(move |store| {
                store.transaction::<_, io::Error>(|mut transaction| {
                    attempt.check_pending(&transaction)?;
                    let update = format!(
                        "DELETE {{ GRAPH <{CONTROL}> {{ {} {} ?version ; {} \
                         ?revision }} }} INSERT {{ GRAPH <{CONTROL}> {{ {} {} \
                         {version} ; {} {} }} }} WHERE {{ GRAPH <{CONTROL}> \
                         {{ {} {} ?version ; {} ?revision }} }}",
                        attempt.id,
                        term("version"),
                        term("revision"),
                        attempt.id,
                        term("version"),
                        term("revision"),
                        oxrdf::Literal::new_simple_literal(&revision),
                        attempt.id,
                        term("version"),
                        term("revision")
                    );
                    transaction
                        .update(update.as_str())
                        .map_err(io::Error::other)?;
                    Ok(())
                })?;
                Ok(())
            })
            .await?;
        self.version = next_version;
        Ok(self)
    }

    /// Atomically publish a complete contribution; unchanged versions are
    /// reused.
    pub async fn complete(self) -> Result<()> {
        let attempt = self.clone();
        self.dataset
            .run(move |store| {
                store.transaction::<_, io::Error>(|mut transaction| {
                    attempt.check_pending(&transaction)?;
                    // Existing versions are immutable. Compare both logical
                    // quad sets.
                    let changed = format!(
                        "ASK {{ GRAPH <{CONTROL}> {{ {} {} ?previous }} {{ \
                         GRAPH <{CONTROL}> {{ ?g {} {} ; {} ?original }} \
                         GRAPH ?g {{ ?s ?p ?o }} FILTER NOT EXISTS {{ GRAPH \
                         <{CONTROL}> {{ ?h {} ?previous ; {} ?original }} \
                         GRAPH ?h {{ ?s ?p ?o }} }} }} UNION {{ GRAPH \
                         <{CONTROL}> {{ ?h {} ?previous ; {} ?original }} \
                         GRAPH ?h {{ ?s ?p ?o }} FILTER NOT EXISTS {{ GRAPH \
                         <{CONTROL}> {{ ?g {} {} ; {} ?original }} GRAPH ?g \
                         {{ ?s ?p ?o }} }} }} }}",
                        attempt.version,
                        term("data"),
                        term("attempt"),
                        attempt.id,
                        term("original"),
                        term("attempt"),
                        term("original"),
                        term("attempt"),
                        term("original"),
                        term("attempt"),
                        attempt.id,
                        term("original")
                    );
                    if let QueryResults::Boolean(true) = transaction
                        .query(changed.as_str())
                        .map_err(io::Error::other)?
                    {
                        return Err(io::Error::other(
                            "same input revision produced different \
                             contribution content",
                        ));
                    }
                    let update = format!(
                        "DELETE {{ GRAPH <{CONTROL}> {{ {} {} ?old . {} {} {} \
                         }} }} INSERT {{ GRAPH <{CONTROL}> {{ {} {} {} }} }} \
                         WHERE {{ OPTIONAL {{ GRAPH <{CONTROL}> {{ {} {} ?old \
                         }} }} }}; INSERT {{ GRAPH <{CONTROL}> {{ {} {} {} }} \
                         }} WHERE {{ FILTER NOT EXISTS {{ GRAPH <{CONTROL}> \
                         {{ {} {} ?existing }} }} }}",
                        attempt.owner,
                        term("head"),
                        attempt.owner,
                        term("pending"),
                        attempt.id,
                        attempt.owner,
                        term("head"),
                        attempt.version,
                        attempt.owner,
                        term("head"),
                        attempt.version,
                        term("data"),
                        attempt.id,
                        attempt.version,
                        term("data")
                    );
                    transaction
                        .update(update.as_str())
                        .map_err(io::Error::other)?;
                    Ok(())
                })?;
                discard_unreferenced(store, &attempt.id)?;
                Ok(())
            })
            .await
    }

    /// Abort without replacing the active contribution, retaining failure
    /// diagnostics.
    pub async fn abort(self) -> Result<()> {
        let attempt = self.clone();
        self.dataset
            .run(move |store| {
                let update = format!(
                    "DELETE {{ GRAPH <{CONTROL}> {{ {} {} {} }} }} INSERT {{ \
                     GRAPH <{CONTROL}> {{ {} {} {} }} }} WHERE {{ GRAPH \
                     <{CONTROL}> {{ {} {} {} }} }}",
                    attempt.owner,
                    term("pending"),
                    attempt.id,
                    attempt.owner,
                    term("failed"),
                    attempt.id,
                    attempt.owner,
                    term("pending"),
                    attempt.id
                );
                store.update(update.as_str())?;
                discard_unreferenced(store, &attempt.id)?;
                Ok(())
            })
            .await
    }
}

fn discard_unreferenced(
    store: &super::backend::BackendStore,
    attempt: &NamedNode,
) -> Result<()> {
    store.transaction::<_, io::Error>(|mut transaction| {
        if transaction
            .quads_for_pattern(
                None,
                Some(term("data").as_ref()),
                Some(attempt.as_ref().into()),
                Some(NamedNode::new_unchecked(CONTROL).as_ref().into()),
            )
            .next()
            .transpose()?
            .is_some()
        {
            return Ok(());
        }
        let mappings = transaction
            .quads_for_pattern(
                None,
                Some(term("attempt").as_ref()),
                Some(attempt.as_ref().into()),
                Some(NamedNode::new_unchecked(CONTROL).as_ref().into()),
            )
            .take(100_001)
            .collect::<io::Result<Vec<_>>>()?;
        ensure_metadata_budget(mappings.len())?;
        for mapping in mappings {
            let oxrdf::Subject::NamedNode(graph) = mapping.subject else {
                return Err(io::Error::other("invalid physical graph mapping"));
            };
            transaction.remove_named_graph(graph.as_ref())?;
            let metadata = transaction
                .quads_for_pattern(
                    Some(graph.as_ref().into()),
                    None,
                    None,
                    Some(NamedNode::new_unchecked(CONTROL).as_ref().into()),
                )
                .take(100_001)
                .collect::<io::Result<Vec<_>>>()?;
            ensure_metadata_budget(metadata.len())?;
            for metadata in metadata {
                transaction.remove(metadata.as_ref())?;
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn ensure_metadata_budget(size: usize) -> io::Result<()> {
    if size > 100_000 {
        Err(io::Error::other(
            "attempt cleanup exceeds 100000 metadata-row budget",
        ))
    } else {
        Ok(())
    }
}
