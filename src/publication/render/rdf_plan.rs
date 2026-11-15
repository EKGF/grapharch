//! The pinned plan of an RDF renderer target: its definition bound to one
//! validated revision, without a scope or profile.
use {
    super::job_subject::JobSubject,
    crate::publication::{
        Renderer,
        assembly::Assembler,
        dataset::{Dataset, identity},
        plan::ResolvedPlan,
        target_links::TargetLinks,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
};

pub(super) struct RdfPlan {
    pub subject:     JobSubject,
    /// The validated revision the plan pins.
    pub source:      RevisionView,
    pub destination: String,
    pub targets:     TargetLinks,
}

impl RdfPlan {
    pub(super) async fn read(
        dataset: Dataset,
        handle: ResolvedPlan,
    ) -> Result<Self> {
        let view = RevisionView::new(dataset.clone(), handle.snapshot.clone());
        ensure!(
            view.has_type(&handle.id, "Plan").await?,
            "missing publication plan {}",
            handle.id
        );
        let project = view.node(&handle.id, "project").await?;
        let target = view.node(&handle.id, "target").await?;
        let revision = view.node(&handle.id, "inputRevision").await?;
        Assembler::new(dataset.clone())
            .require_validated(project.as_str(), &revision)
            .await?;
        ensure!(
            handle.id ==
                identity("publication-plan", &[
                    project.as_str(),
                    target.as_str(),
                    revision.as_str()
                ]),
            "publication plan identity disagrees with its provenance"
        );
        let source = RevisionView::new(dataset.clone(), revision.clone());
        let renderer = view.renderer(&handle.id).await?;
        let destination = view.text(&handle.id, "destination").await?;
        ensure!(
            renderer == Renderer::Rdf &&
                source.renderer(&target).await? == renderer &&
                source.text(&target, "destination").await? == destination,
            "plan does not match its pinned target definition"
        );
        let targets =
            TargetLinks::read(&dataset, &source, &project, &target, None)
                .await?;
        Ok(Self {
            subject: JobSubject {
                dataset,
                handle,
                project,
                target,
                revision,
                renderer,
            },
            source,
            destination,
            targets,
        })
    }
}
