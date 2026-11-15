//! Plans of targets that publish named files from collected sources rather
//! than arranging a scope: an RDF target's plan pins its definition to the
//! validated revision, and the renderer reads the sources from that revision.
use {
    super::{ResolvedPlan, TargetPlanner},
    crate::publication::{
        RDF_TYPE,
        dataset::{Stage, identity},
        rdf_target::RdfTarget,
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::Result,
    oxrdf::{Literal, NamedNode, Quad, Term},
};

impl TargetPlanner {
    /// Validate an RDF target against the revision and record its plan.
    pub(super) async fn resolve_files(
        &self,
        view: &RevisionView,
        project: &NamedNode,
        target: &NamedNode,
        renderer: crate::publication::Renderer,
        writer: &GraphWriter,
    ) -> Result<ResolvedPlan> {
        // Fail at planning on an invalid definition, before any rendering.
        RdfTarget::read(view, project, target).await?;
        let destination = view.text(target, "destination").await?;
        let plan = identity("publication-plan", &[
            project.as_str(),
            target.as_str(),
            self.revision.as_str(),
        ]);
        let mut stream = writer
            .begin_stage(
                project.as_str(),
                "urn:grapharch:target-planner:v2",
                target.as_str(),
                self.revision.as_str(),
                Stage::Plan,
            )
            .await?;
        for (predicate, object) in [
            (
                NamedNode::new_unchecked(RDF_TYPE),
                Term::NamedNode(term("Plan")),
            ),
            (term("project"), project.clone().into()),
            (term("target"), target.clone().into()),
            (
                term("inputRevision"),
                self.revision.clone().into(),
            ),
            (term("renderer"), renderer.iri().into()),
            (
                term("destination"),
                Literal::new_simple_literal(destination).into(),
            ),
        ] {
            stream
                .write(Quad::new(
                    plan.clone(),
                    predicate,
                    object,
                    plan.clone(),
                ))
                .await?;
        }
        let receipt = stream.commit().await?;
        let snapshot = self.dataset.pin(&receipt).await?;
        Ok(ResolvedPlan { id: plan, snapshot })
    }
}
