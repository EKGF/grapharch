//! Validation and atomic promotion of complete, project-owned source revisions.

use {
    super::{
        dataset::{CONTROL, Dataset},
        term,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxigraph::sparql::QueryResults,
    oxrdf::{NamedNode, Quad},
    std::io,
};

/// Assembles source/document contributions without incorporating derived plans.
pub struct Assembler {
    dataset: Dataset,
}

impl Assembler {
    /// Attach to the same dataset used by collection and documentors.
    pub fn new(dataset: Dataset) -> Self { Self { dataset } }

    /// Capture, validate, then promote only if inputs are still complete and
    /// unchanged.
    pub async fn assemble(&self, project: &str) -> Result<NamedNode> {
        let snapshot = self.dataset.snapshot_for(project).await?;
        let view = RevisionView::new(self.dataset.clone(), snapshot.clone());
        ensure!(
            view.has_type(&NamedNode::new(project)?, "Project").await?,
            "undefined project {project}"
        );
        view.validate().await?;
        self.promote(project, &snapshot).await?;
        Ok(snapshot)
    }

    /// Validate a historical publication revision without requiring current
    /// sources.
    pub async fn require_validated(
        &self,
        project: &str,
        revision: &NamedNode,
    ) -> Result<()> {
        let project = NamedNode::new(project)?;
        let revision = revision.clone();
        self.dataset
            .run(move |store| {
                ensure!(
                    store.contains(
                        Quad::new(
                            revision,
                            term("validatedFor"),
                            project,
                            NamedNode::new_unchecked(CONTROL)
                        )
                        .as_ref()
                    )?,
                    "revision has not been validated for this project"
                );
                Ok(())
            })
            .await
    }

    /// Read the last successfully assembled revision, unaffected by invalid
    /// edits.
    pub async fn current(&self, project: &str) -> Result<Option<NamedNode>> {
        let project = NamedNode::new(project)?;
        self.dataset
            .run(move |store| {
                let mut rows = store.quads_for_pattern(
                    Some(project.as_ref().into()),
                    Some(term("currentRevision").as_ref()),
                    None,
                    Some(NamedNode::new_unchecked(CONTROL).as_ref().into()),
                );
                let value = rows.next().transpose()?.map(|quad| quad.object);
                ensure!(
                    rows.next().transpose()?.is_none(),
                    "conflicting current revisions"
                );
                match value {
                    None => Ok(None),
                    Some(oxrdf::Term::NamedNode(node)) => Ok(Some(node)),
                    _ => anyhow::bail!("invalid current revision pointer"),
                }
            })
            .await
    }

    async fn promote(&self, project: &str, revision: &NamedNode) -> Result<()> {
        let project = NamedNode::new(project)?;
        let revision = revision.clone();
        self.dataset
            .run(move |store| {
                store.transaction::<_, io::Error>(|mut transaction| {
                    let blocked = format!(
                        "ASK {{ GRAPH <{CONTROL}> {{ ?owner ?state ?attempt . \
                         ?attempt {} {project} ; {} ?stage . VALUES ?state {{ \
                         {} {} }} VALUES ?stage {{ \"collect\" \"document\" \
                         }} }} }}",
                        term("project"),
                        term("stage"),
                        term("pending"),
                        term("failed")
                    );
                    if matches!(
                        transaction
                            .query(blocked.as_str())
                            .map_err(io::Error::other)?,
                        QueryResults::Boolean(true)
                    ) {
                        return Err(io::Error::other(
                            "project inputs are pending or failed",
                        ));
                    }
                    let current = input_heads(&project);
                    // Symmetric set difference on (owner, version). Correlated
                    // NOT EXISTS makes Oxigraph repeatedly scan the head set,
                    // producing a quadratic Cartesian join on large families.
                    let changed = format!(
                        "ASK {{ {{ GRAPH {revision} {{ ?owner {} ?version }} \
                         MINUS {{ {current} }} }} UNION {{ {current} MINUS {{ \
                         GRAPH {revision} {{ ?owner {} ?version }} }} }} }}",
                        term("head"),
                        term("head")
                    );
                    if matches!(
                        transaction
                            .query(changed.as_str())
                            .map_err(io::Error::other)?,
                        QueryResults::Boolean(true)
                    ) {
                        return Err(io::Error::other(
                            "source contributions changed during assembly",
                        ));
                    }
                    let update = format!(
                        "DELETE {{ GRAPH <{CONTROL}> {{ {project} {} \
                         ?previous }} }} INSERT {{ GRAPH <{CONTROL}> {{ \
                         {project} {} {revision} . {revision} {} {project} }} \
                         }} WHERE {{ OPTIONAL {{ GRAPH <{CONTROL}> {{ \
                         {project} {} ?previous }} }} }}",
                        term("currentRevision"),
                        term("currentRevision"),
                        term("validatedFor"),
                        term("currentRevision")
                    );
                    transaction
                        .update(update.as_str())
                        .map_err(io::Error::other)?;
                    Ok(())
                })?;
                Ok(())
            })
            .await
    }
}

fn input_heads(project: &NamedNode) -> String {
    format!(
        "GRAPH <{CONTROL}> {{ ?owner {} ?version . ?version {} ?attempt . \
         ?attempt {} {project} ; {} ?stage . VALUES ?stage {{ \"collect\" \
         \"document\" }} }}",
        term("head"),
        term("data"),
        term("project"),
        term("stage")
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::publication::{
            collect::{Collector, RdfFile},
            dataset::Stage,
            writer::GraphWriter,
        },
    };

    #[tokio::test]
    async fn promotion_compares_thousands_of_heads_without_a_cartesian_join()
    -> Result<()> {
        let dataset = Dataset::memory()?;
        let revision = NamedNode::new("urn:large:revision")?;
        dataset
            .run(|store| {
                for n in 0..4096 {
                    let owner = NamedNode::new(format!("urn:large:owner:{n}"))?;
                    let version =
                        NamedNode::new(format!("urn:large:version:{n}"))?;
                    let attempt =
                        NamedNode::new(format!("urn:large:attempt:{n}"))?;
                    for quad in [
                        Quad::new(
                            owner.clone(),
                            term("head"),
                            version.clone(),
                            NamedNode::new_unchecked(CONTROL),
                        ),
                        Quad::new(
                            version.clone(),
                            term("data"),
                            attempt.clone(),
                            NamedNode::new_unchecked(CONTROL),
                        ),
                        Quad::new(
                            attempt.clone(),
                            term("project"),
                            NamedNode::new("urn:large:project")?,
                            NamedNode::new_unchecked(CONTROL),
                        ),
                        Quad::new(
                            attempt,
                            term("stage"),
                            oxrdf::Literal::new_simple_literal("document"),
                            NamedNode::new_unchecked(CONTROL),
                        ),
                        Quad::new(
                            owner,
                            term("head"),
                            version,
                            NamedNode::new("urn:large:revision")?,
                        ),
                    ] {
                        store.insert(quad.as_ref())?;
                    }
                }
                Ok(())
            })
            .await?;
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            Assembler::new(dataset).promote("urn:large:project", &revision),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "head comparison must not perform a quadratic Cartesian join"
            )
        })??;
        Ok(())
    }

    #[tokio::test]
    async fn promotion_rechecks_both_head_changes_and_incomplete_inputs()
    -> Result<()> {
        let project = "urn:example:ontology-project";
        let dataset = Dataset::memory()?;
        let (writer, task) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        RdfFile::new(
            project,
            "urn:collector:definitions",
            "tests/fixtures/publication/projects.ttl".into(),
        )?
        .collect(&writer)
        .await?;
        let assembler = Assembler::new(dataset.clone());
        let revision = assembler.assemble(project).await?;
        // A pending derived plan is not a pending source input.
        let derived = dataset
            .begin_stage(
                project,
                "urn:planner",
                "target",
                "r1",
                Stage::Plan,
            )
            .await?;
        assembler.promote(project, &revision).await?;
        derived.abort().await?;
        let added = dataset
            .begin(project, "urn:new-collector", "source", "r1")
            .await?;
        assert!(assembler.promote(project, &revision).await.is_err());
        added.complete().await?;
        assert!(assembler.promote(project, &revision).await.is_err());
        assert_eq!(Some(revision), assembler.current(project).await?);
        drop(writer);
        task.await??;
        Ok(())
    }
}
