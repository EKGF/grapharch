//! Exact-revision publication. GraphArch definitions come from a verified
//! controlling revision; only content comes from the designated commit, so a
//! designated branch never needs, and cannot supply, GraphArch configuration.
use {
    super::{
        assembly::Assembler,
        collect::{
            configured_git,
            git::{CommittedDefinitions, RepositoryScan},
        },
        dataset::Dataset,
        document::{
            Documentor,
            VocabularyReferenceDocumentor,
            kind::DocumentorKind,
            native_metadata,
            scope_inclusion,
        },
        plan::{ResolvedPlan, TargetPlanner},
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

mod request;
mod selection;
mod trust;
pub use {
    request::{BuildManifest, BuildRequest},
    selection::TargetSelection,
};

/// Processor of the controlling definitions contribution.
pub const DEFINITIONS_PROCESSOR: &str = "urn:grapharch:publication-definitions";
const COLLECTOR_CONCURRENCY: usize = 4;
const DOCUMENTOR_USES: usize = 8;

/// Everything one revision build reads: one exact content tree and one
/// verified definitions file, both bound to the same project.
pub struct RevisionInputs {
    /// Content pinned at the designated commit; read as data only.
    pub content:     RepositoryScan,
    /// Definitions pinned at the controlling commit.
    pub definitions: CommittedDefinitions,
    /// Project defined by the controlling definitions.
    pub project:     NamedNode,
    /// Targets to resolve.
    pub targets:     TargetSelection,
}

/// Collect content through the configured collectors, generate configured
/// documentation, assemble one revision and resolve the selected targets.
pub async fn collect(
    inputs: RevisionInputs,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    ensure!(
        inputs.content.project() == inputs.project.as_str() &&
            inputs.definitions.project() == inputs.project.as_str(),
        "content and definitions must be pinned for project {}",
        inputs.project
    );
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let result = Build {
        inputs:  &inputs,
        dataset: &dataset,
        writer:  &writer,
    }
    .run()
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}

struct Build<'a> {
    inputs:  &'a RevisionInputs,
    dataset: &'a Dataset,
    writer:  &'a GraphWriter,
}

impl Build<'_> {
    async fn run(&self) -> Result<Vec<ResolvedPlan>> {
        let project = &self.inputs.project;
        self.inputs.content.provenance.collect(self.writer).await?;
        self.inputs
            .definitions
            .provenance()
            .collect(self.writer)
            .await?;
        self.inputs
            .definitions
            .blob()
            .rdf_collector(DEFINITIONS_PROCESSOR)?
            .collect(self.writer)
            .await?;
        let definitions = RevisionView::new(
            self.dataset.clone(),
            self.dataset
                .snapshot_for_processor(project.as_str(), DEFINITIONS_PROCESSOR)
                .await?,
        );
        ensure!(
            definitions.has_type(project, "Project").await?,
            "definitions {} at {} do not declare {project} as a ga:Project",
            self.inputs.definitions.path(),
            self.inputs.definitions.commit()
        );
        let targets =
            self.inputs.targets.resolve(&definitions, project).await?;
        configured_git::collect(
            &self.inputs.content,
            &definitions,
            project,
            self.writer,
            COLLECTOR_CONCURRENCY,
        )
        .await?;
        let collected = self.dataset.snapshot_for(project.as_str()).await?;
        trust::reject_content_definitions(
            self.dataset,
            &definitions,
            &collected,
        )
        .await?;
        scope_inclusion::bind(self.dataset, &definitions, project, self.writer)
            .await?;
        self.document(&definitions).await?;
        native_metadata::derive(
            self.dataset,
            &definitions,
            project,
            self.writer,
        )
        .await?;
        for target in &targets {
            if definitions.renderer(target).await? == super::Renderer::Website {
                super::website::bind(
                    self.dataset,
                    self.writer,
                    project.as_str(),
                    target.as_str(),
                )
                .await?;
            }
        }
        let revision = Assembler::new(self.dataset.clone())
            .assemble(project.as_str())
            .await?;
        let planner = TargetPlanner::new(self.dataset.clone(), revision);
        let mut plans = Vec::with_capacity(targets.len());
        for target in &targets {
            plans.push(
                planner
                    .resolve(project.as_str(), target, self.writer)
                    .await?,
            );
        }
        Ok(plans)
    }

    /// Run the project's configured documentor uses, read from the
    /// definitions only.
    async fn document(&self, definitions: &RevisionView) -> Result<()> {
        let project = &self.inputs.project;
        let mut rows = definitions.objects(project, "documentor").await?;
        let mut uses = Vec::new(); // bounded: DOCUMENTOR_USES.
        while let Some(row) = rows.next().await? {
            ensure!(
                uses.len() < DOCUMENTOR_USES,
                "{project} exceeds the {DOCUMENTOR_USES}-documentor budget"
            );
            let Term::NamedNode(processor) = &row["value"] else {
                bail!("{project} ga:documentor must identify an IRI")
            };
            uses.push(processor.clone());
        }
        uses.sort();
        for processor in uses {
            ensure!(
                definitions.has_type(&processor, "DocumentorUse").await?,
                "documentor {processor} must be typed ga:DocumentorUse"
            );
            let kind = DocumentorKind::from_node(
                &definitions.node(&processor, "documentorType").await?,
            )?;
            ensure!(
                kind == DocumentorKind::VocabularyReference,
                "documentor {processor} uses {kind:?}, which a revision build \
                 does not run"
            );
            VocabularyReferenceDocumentor::new(
                self.dataset.clone(),
                definitions.clone(),
                project,
                processor.clone(),
            )
            .await?
            .document(self.writer)
            .await
            .with_context(|| format!("documentor {processor}"))?;
        }
        Ok(())
    }
}
