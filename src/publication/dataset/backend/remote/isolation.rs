//! Proof at connect time that the store isolates transactions SERIALIZABLY.
use {
    super::{commit::Conflict, store::RemoteStore},
    crate::publication::sparql::{Drain, LimitedRead, http_error, status},
    anyhow::{Result, ensure},
    oxigraph::sparql::{
        QueryResults,
        results::{QueryResultsFormat, QueryResultsParser},
    },
    oxrdf::{Literal, NamedNode, Quad},
    std::{io, sync::Arc},
};

impl RemoteStore {
    pub(super) fn verify_isolation(self: &Arc<Self>) -> Result<()> {
        let graph = NamedNode::new(format!(
            "urn:grapharch:isolation-probe:{}",
            uuid::Uuid::new_v4()
        ))?;
        let first = self.session()?;
        let second = self.session()?;
        let result = (|| -> Result<()> {
            let query = format!("ASK {{ GRAPH {graph} {{ ?s ?p ?o }} }}");
            for session in [&first, &second] {
                ensure!(
                    matches!(
                        session.query(&query)?,
                        QueryResults::Boolean(false)
                    ),
                    "isolation probe graph is not empty"
                );
            }
            let predicate =
                NamedNode::new("urn:grapharch:isolation-probe:predicate")?;
            first.insert(
                Quad::new(
                    NamedNode::new("urn:grapharch:isolation-probe:first")?,
                    predicate.clone(),
                    Literal::new_simple_literal("first"),
                    graph.clone(),
                )
                .as_ref(),
            )?;
            second.insert(
                Quad::new(
                    NamedNode::new("urn:grapharch:isolation-probe:second")?,
                    predicate,
                    Literal::new_simple_literal("second"),
                    graph.clone(),
                )
                .as_ref(),
            )?;
            // The writes are disjoint. SNAPSHOT permits both; SERIALIZABLE
            // must reject the predicate-range/phantom conflict.
            first.commit()?;
            // Both configured endpoints must address the same repository.
            let mut response = self
                .client
                .post(&self.config.query_endpoint)
                .header("Content-Type", "application/sparql-query")
                .header("Accept", "application/sparql-results+json")
                .body(query.clone())
                .send()
                .map_err(http_error)?;
            if let Err(error) = status(response.status()) {
                response.drain()?;
                return Err(error.into());
            }
            let mut body =
                LimitedRead::new(response, self.config.http.max_response_bytes);
            let same = matches!(
                QueryResultsParser::from_format(QueryResultsFormat::Json)
                    .for_reader(&mut body)?
                    .into(),
                QueryResults::Boolean(true)
            );
            body.drain()?;
            ensure!(
                same,
                "query and transaction endpoints do not expose the same \
                 committed repository"
            );
            let rejected = second.commit().err().is_some_and(|e| {
                e.get_ref().is_some_and(|e| e.is::<Conflict>())
            });
            ensure!(
                rejected,
                "SPARQL backend did not demonstrate SERIALIZABLE \
                 predicate-conflict isolation"
            );
            Ok(())
        })();
        let rollback_first = first.rollback();
        let rollback_second = second.rollback();
        let cleanup = (|| -> io::Result<()> {
            let tx = self.session()?;
            tx.update(&format!("DROP SILENT GRAPH {graph}"))?;
            tx.commit()
        })();
        rollback_first?;
        rollback_second?;
        cleanup?;
        result
    }
}
