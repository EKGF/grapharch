//! Native semantic showcases use the same reusable documentors as library
//! users.
use {
    super::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::{Dataset, Stage, identity},
        document::{Documentor, UseCaseDocumentor},
        plan::{ResolvedPlan, TargetPlanner},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    std::{path::Path, sync::Arc},
};
pub(super) async fn skos(
    source: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let built = super::ontology_family::collect(
        &source.join("sources"),
        &source.join("project.ttl"),
        "urn:skos:project",
        "urn:skos:specification",
        4,
    )
    .await?;
    Ok((built.dataset, vec![built.plan]))
}
pub(super) async fn use_cases(
    source: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let project = "urn:use-cases:project";
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let result = async {
        let mut work: Vec<Arc<dyn Collector>> = Vec::new(); // bounded: exactly two authored fixture inputs.
        for file in ["project.ttl", "use-cases.ttl"] {
            work.push(Arc::new(RdfFile::new(
                project,
                identity("use-case-showcase-collector", &[file]).as_str(),
                source.join(file),
            )?));
        }
        collect_all(work, &writer, 2).await?;
        let snapshot = dataset.snapshot_collected_for(project).await?;
        let view = RevisionView::new(dataset.clone(), snapshot.clone());
        let documentor =
            UseCaseDocumentor::new(dataset.clone(), snapshot.clone(), project)?;
        documentor.document(&writer).await?;
        let subjects = documentor.subjects().await?;
        ensure!(
            !subjects.is_empty(),
            "use-case showcase requires explicit EKGF UseCase instances"
        );
        let mut stream = writer
            .begin_stage(
                project,
                "urn:use-cases:bindings",
                project,
                &format!("bindings-v1:{snapshot}"),
                Stage::Document,
            )
            .await?;
        let profile = NamedNode::new("urn:use-cases:profile")?;
        let scope = NamedNode::new("urn:use-cases:scope")?;
        for (order, subject) in subjects.iter().enumerate() {
            let component = identity("use-case-component", &[subject.as_str()]);
            let placement = identity("use-case-placement", &[subject.as_str()]);
            let title = view
                .optional_by(
                    subject,
                    &NamedNode::new_unchecked(
                        "http://www.w3.org/2000/01/rdf-schema#label",
                    ),
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("use case lacks a title"))?;
            for quad in [
                Quad::new(
                    scope.clone(),
                    term("include"),
                    subject.clone(),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    profile.clone(),
                    term("placement"),
                    placement.clone(),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    placement.clone(),
                    NamedNode::new_unchecked(super::RDF_TYPE),
                    term("Placement"),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    placement.clone(),
                    term("component"),
                    component.clone(),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    placement,
                    term("order"),
                    Literal::from(order as u32),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    component.clone(),
                    NamedNode::new_unchecked(super::RDF_TYPE),
                    term("Component"),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    component.clone(),
                    NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                    title,
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    component,
                    term("fragment"),
                    documentor.fragment_id(subject),
                    GraphName::DefaultGraph,
                ),
            ] {
                stream.write(quad).await?;
            }
        }
        stream.complete().await?;
        let revision =
            Assembler::new(dataset.clone()).assemble(project).await?;
        let mut plans = Vec::new(); // bounded: the two fixture targets.
        for target in ["urn:use-cases:website", "urn:use-cases:pdf"] {
            plans.push(
                TargetPlanner::new(dataset.clone(), revision.clone())
                    .resolve(project, &NamedNode::new(target)?, &writer)
                    .await?,
            );
        }
        Ok::<_, anyhow::Error>(plans)
    }
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}
