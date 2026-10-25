use {
    super::{CONTROL, CommittedContribution, Dataset, identity},
    crate::publication::term,
    anyhow::{Result, ensure},
    oxigraph::sparql::QueryResults,
    oxrdf::{NamedNode, Quad},
    sha2::{Digest, Sha256},
    uuid::Uuid,
};

impl Dataset {
    /// Pin all current contribution heads in one atomic SPARQL update.
    /// Snapshots contain only metadata pointers; source assertions are not
    /// copied.
    pub async fn snapshot(&self) -> Result<NamedNode> {
        self.snapshot_matching(String::new()).await
    }

    /// Pin only this project's collected/documented contributions, excluding
    /// plans.
    pub async fn snapshot_for(&self, project: &str) -> Result<NamedNode> {
        let project = NamedNode::new(project)?;
        let filter = format!(
            "?version {} ?attempt . ?attempt {} {project} ; {} ?stage . \
             VALUES ?stage {{ \"collect\" \"document\" }}",
            term("data"),
            term("project"),
            term("stage")
        );
        self.snapshot_matching(filter).await
    }

    /// Pin source assertions without feeding generated documentation back into
    /// a documentor's input revision.
    pub(crate) async fn snapshot_collected_for(
        &self,
        project: &str,
    ) -> Result<NamedNode> {
        let project = NamedNode::new(project)?;
        self.snapshot_matching(format!(
            "?version {} ?attempt . ?attempt {} {project} ; {} \"collect\"",
            term("data"),
            term("project"),
            term("stage")
        ))
        .await
    }

    /// Pin collected assertions owned by one source processor, so discovering
    /// ontology declarations cannot accidentally select another source.
    pub(crate) async fn snapshot_for_processor(
        &self,
        project: &str,
        processor: &str,
    ) -> Result<NamedNode> {
        let project = NamedNode::new(project)?;
        let processor = NamedNode::new(processor)?;
        self.snapshot_matching(format!(
            "?version {} ?attempt . ?attempt {} {project} ; {} {processor} ; \
             {} \"collect\"",
            term("data"),
            term("project"),
            term("collector"),
            term("stage")
        ))
        .await
    }

    async fn snapshot_matching(&self, filter: String) -> Result<NamedNode> {
        self.run(move |store| {
            // A staging snapshot is captured by one transactional update.
            let capture = NamedNode::new(format!(
                "urn:grapharch:capture:{}",
                Uuid::new_v4()
            ))?;
            store.update(
                format!(
                    "INSERT {{ GRAPH {capture} {{ ?owner {} ?version }} }} \
                     WHERE {{ GRAPH <{CONTROL}> {{ ?owner {} ?version . \
                     {filter} }} }}",
                    term("head"),
                    term("head")
                )
                .as_str(),
            )?;
            let mut digest = Sha256::new();
            if let QueryResults::Solutions(rows) = store.query(
                format!(
                    "SELECT ?owner ?version WHERE {{ GRAPH {capture} {{ \
                     ?owner {} ?version }} }} ORDER BY ?owner ?version",
                    term("head")
                )
                .as_str(),
            )? {
                for row in rows {
                    let row = row?;
                    digest.update(format!(
                        "{} {}\n",
                        row["owner"], row["version"]
                    ));
                }
            }
            let revision = NamedNode::new(format!(
                "urn:grapharch:snapshot:{:x}",
                digest.finalize()
            ))?;
            store.update(
                format!(
                    "INSERT {{ GRAPH {revision} {{ ?s ?p ?o }} }} WHERE {{ \
                     GRAPH {capture} {{ ?s ?p ?o }} }}; DROP SILENT GRAPH \
                     {capture}"
                )
                .as_str(),
            )?;
            Ok(revision)
        })
        .await
    }

    /// Pin an acknowledged contribution directly, even after its head is
    /// superseded.
    pub async fn pin(
        &self,
        receipt: &CommittedContribution,
    ) -> Result<NamedNode> {
        let receipt = receipt.clone();
        self.run(move |store| {
            ensure!(
                store
                    .quads_for_pattern(
                        Some(receipt.version.as_ref().into()),
                        Some(term("data").as_ref()),
                        None,
                        Some(NamedNode::new_unchecked(CONTROL).as_ref().into())
                    )
                    .next()
                    .transpose()?
                    .is_some(),
                "contribution has not committed"
            );
            let snapshot = identity("pinned-contribution", &[
                receipt.owner.as_str(),
                receipt.version.as_str(),
            ]);
            store.insert(
                Quad::new(
                    receipt.owner,
                    term("head"),
                    receipt.version,
                    snapshot.clone(),
                )
                .as_ref(),
            )?;
            Ok(snapshot)
        })
        .await
    }
}
