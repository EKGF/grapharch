//! Shared showcase pipeline: executable examples and integration tests call
//! exactly the same collection, planning, and publishing code.
use {
    crate::publication::{
        assembly::Assembler,
        collect::{
            Collector,
            MarkdownFile,
            RdfFile,
            SparqlFile,
            W3cSpecificationFile,
            collect_all,
        },
        dataset::Dataset,
        document::{Documentor, OwlClassDocumentor},
        plan::{ResolvedPlan, TargetPlanner},
        render::{Publication, PublishOutcome, Publisher},
        term,
        writer::GraphWriter,
    },
    anyhow::{Result, bail},
    oxrdf::{GraphName, NamedNode, Quad},
    std::{path::Path, sync::Arc},
};

/// Collect a registry-selected EKGF ontology family.
pub async fn family(source: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let built = super::ontology_family::collect(
        &source.join("sources"),
        &source.join("project.ttl"),
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await?;
    Ok((built.dataset, vec![built.plan]))
}

/// Collect the pinned native DPROD ontology, shapes, and authored fragments.
pub async fn dprod(source: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    super::showcases_dprod::collect(source).await
}

pub async fn authored(source: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let project = "urn:showcase:project";
    RdfFile::new(
        project,
        "urn:showcase:collector",
        source.to_owned(),
    )?
    .collect(&writer)
    .await?;
    let plans = plan(&dataset, &writer, project, &[
        "urn:showcase:spec",
        "urn:showcase:reader",
        "urn:showcase:pdf",
    ])
    .await?;
    drop(writer);
    worker.await??;
    Ok((dataset, plans))
}

pub async fn specification(
    source: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let project = "urn:specification:project";
    RdfFile::new(
        project,
        "urn:specification:collector",
        source.to_owned(),
    )?
    .collect(&writer)
    .await?;
    let plans = plan(&dataset, &writer, project, &[
        "urn:specification:target",
    ])
    .await?;
    drop(writer);
    worker.await??;
    Ok((dataset, plans))
}

pub async fn w3c(sources: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    #[derive(serde::Deserialize)]
    struct Reference {
        source: String,
    }
    let reference: Reference = serde_yaml::from_str(
        &tokio::fs::read_to_string(sources.join("reference.yaml")).await?,
    )?;
    let project = "urn:w3c-example:project";
    let collector = W3cSpecificationFile::new(
        project,
        "urn:w3c-example:collector",
        sources.join("source.html"),
        &reference.source,
    )?;
    collector.collect(&writer).await?;
    RdfFile::new(
        project,
        "urn:w3c-example:authority-assets",
        sources.join("authority.ttl"),
    )?
    .collect(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, &collector.target_id(), &writer)
        .await?;
    drop(writer);
    worker.await??;
    Ok((dataset, vec![plan]))
}

pub async fn mixed(sources: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let project = "urn:mixed:project";
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let markdown = Arc::new(MarkdownFile::new(
        project,
        "urn:mixed:markdown",
        sources.join("introduction.md"),
    )?);
    let sparql = Arc::new(SparqlFile::new(
        project,
        "urn:mixed:sparql",
        sources.join("query.rq"),
    )?);
    let work: Vec<Arc<dyn Collector>> = vec![
        markdown.clone(),
        sparql.clone(),
        Arc::new(RdfFile::new(
            project,
            "urn:mixed:ontology",
            sources.join("ontology.ttl"),
        )?),
        Arc::new(RdfFile::new(
            project,
            "urn:mixed:definitions",
            sources.join("project.ttl"),
        )?),
    ];
    collect_all(work, &writer, 4).await?;
    let documentor = OwlClassDocumentor::new(
        dataset.clone(),
        dataset.snapshot().await?,
        project,
    )?;
    documentor.document(&writer).await?;

    // The example explicitly binds publication components to the adapters'
    // stable fragment identities. This is fixture wiring, not automatic
    // repository discovery or a general project-configuration implementation.
    let bindings = [
        (
            "urn:mixed:introduction-component",
            markdown.fragment_id(),
        ),
        ("urn:mixed:query-component", sparql.fragment_id()),
        (
            "urn:mixed:class-component",
            documentor.fragment_id(&NamedNode::new("urn:mixed:DataProduct")?),
        ),
    ];
    let mut stream = writer
        .begin(
            project,
            "urn:mixed:bindings",
            "urn:mixed:bindings",
            "bindings-v1",
        )
        .await?;
    for (component, fragment) in bindings {
        stream
            .write(Quad::new(
                NamedNode::new(component)?,
                term("fragment"),
                fragment,
                GraphName::DefaultGraph,
            ))
            .await?;
    }
    stream.complete().await?;
    let plans = plan(&dataset, &writer, project, &[
        "urn:mixed:website",
        "urn:mixed:pdf",
    ])
    .await?;
    drop(writer);
    worker.await??;
    Ok((dataset, plans))
}

async fn plan(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    targets: &[&str],
) -> Result<Vec<ResolvedPlan>> {
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let mut plans = Vec::with_capacity(targets.len());
    for target in targets {
        plans.push(
            planner
                .resolve(project, &NamedNode::new(*target)?, writer)
                .await?,
        );
    }
    Ok(plans)
}

pub async fn publish(
    publisher: &Publisher,
    plans: &[ResolvedPlan],
    root: &Path,
) -> Result<Vec<Publication>> {
    let mut publications = Vec::with_capacity(plans.len());
    for plan in plans {
        match publisher.render(plan, root).await? {
            PublishOutcome::Published(publication) => {
                publications.push(*publication)
            },
            PublishOutcome::Superseded => {
                bail!("showcase publication was superseded")
            },
        }
    }
    Ok(publications)
}
