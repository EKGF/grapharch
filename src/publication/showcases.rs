//! Shared showcase pipeline: executable examples and integration tests call
//! exactly the same collection, planning, and publishing code.
use {
    super::showcase_configuration::ShowcaseConfiguration,
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
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, bail},
    oxrdf::{GraphName, NamedNode, Quad},
    std::{path::Path, sync::Arc},
};

pub use super::{
    gallery::{GalleryReport, ShowcaseReport},
    showcases_mkdocs::build as mkdocs,
};

/// Run every declared live showcase and retain structured diagnostics and KG
/// evidence.
pub async fn verify_gallery(
    dataset: Dataset,
    root: &Path,
    project: &str,
    output: &Path,
) -> Result<GalleryReport> {
    super::gallery::verify(dataset, root, project, output).await
}

/// The GraphArch settings of an ontology-family showcase.
const FAMILY_SETTINGS: &[&str] = &[
    "gitRepository",
    "gitRef",
    "hintsPath",
    "rdfDocument",
    "definitionsPath",
    "showcaseProject",
    "showcaseTargets",
];

/// Verify every copied fixture file a showcase configuration records against
/// its SHA-256 digest, returning the verified files.
pub async fn verify_copied_sources(
    configuration: &Path,
) -> Result<Vec<std::path::PathBuf>> {
    let configuration = ShowcaseConfiguration::read(configuration).await?;
    let copied = configuration.copied_sources()?;
    anyhow::ensure!(
        !copied.is_empty(),
        "{} records no copied sources",
        configuration.path().display()
    );
    // bounded: at most 64 recorded copies.
    let mut verified = Vec::with_capacity(copied.len());
    for source in copied {
        source.verify().await?;
        verified.push(source.path().to_owned());
    }
    Ok(verified)
}

/// Collect an explicitly configured repository family through the shared Git
/// cache. A moving ref is freshly resolved by the collector; its concrete
/// commit is retained in the KG.
pub async fn family(
    source: &Path,
    cache: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    family_configured(&ShowcaseConfiguration::read(source).await?, cache).await
}

/// Local definitions overriding a family's own publication hints: a
/// definitions file beside the configuration, its project, and its ordered
/// targets, of which the first is the family publication.
struct LocalDefinitions {
    path:       std::path::PathBuf,
    project:    String,
    target:     String,
    additional: Vec<String>,
}

impl LocalDefinitions {
    async fn read(
        configuration: &ShowcaseConfiguration,
    ) -> Result<Option<Self>> {
        let path = configuration.optional_text("definitionsPath")?;
        let project = configuration.optional_node("showcaseProject")?;
        match (path, project) {
            (None, None) => Ok(None),
            (Some(_), Some(project)) => {
                let targets = configuration
                    .node_list("showcaseTargets")?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "family showcase {} with local definitions \
                             requires ga:showcaseTargets",
                            configuration.id()
                        )
                    })?;
                let mut targets =
                    targets.into_iter().map(NamedNode::into_string);
                let target = targets.next().ok_or_else(|| {
                    anyhow::anyhow!("family showcase targets cannot be empty")
                })?;
                Ok(Some(Self {
                    path: configuration.local_file("definitionsPath").await?,
                    project: project.into_string(),
                    target,
                    additional: targets.collect(),
                }))
            },
            _ => {
                bail!(
                    "family showcase {} requires ga:definitionsPath and \
                     ga:showcaseProject together",
                    configuration.id()
                )
            },
        }
    }
}

/// The repository-hinted targets a showcase without local definitions
/// publishes: its ordered ga:showcaseTargets, or every registered target.
fn hinted_selection(
    configuration: &ShowcaseConfiguration,
) -> Result<super::collect::git::TargetSelection> {
    use super::collect::git::TargetSelection;
    match configuration.node_list("showcaseTargets")? {
        None => Ok(TargetSelection::All),
        Some(targets) => TargetSelection::targets(targets),
    }
}

pub(super) async fn family_configured(
    configuration: &ShowcaseConfiguration,
    cache: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    use anyhow::ensure;
    configuration.accept_only("ontology-family", FAMILY_SETTINGS)?;
    let documents = configuration.nodes("rdfDocument")?;
    let repository = configuration.optional_text("gitRepository")?;
    let local = LocalDefinitions::read(configuration).await?;
    if !documents.is_empty() {
        ensure!(
            repository.is_none(),
            "family showcase {} reads either RDF documents or a Git \
             repository, not both",
            configuration.id()
        );
        let Some(local) = local else {
            anyhow::bail!(
                "RDF URL family requires definitions, project and target"
            );
        };
        let documents = documents
            .iter()
            .map(|document| {
                Ok(super::ontology_family::RdfUrlFamilyCollector {
                    url:        document.as_str().to_owned(),
                    media_type: configuration.text_of(document, "format")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let built = super::ontology_family::collect_from(
            Dataset::memory()?,
            super::ontology_family::FamilySource::RdfUrls(&documents),
            &local.path,
            &local.project,
            &local.target,
            4,
        )
        .await?;
        return family_targets(built, &local.additional).await;
    }
    let url = repository.ok_or_else(|| {
        anyhow::anyhow!(
            "family showcase {} requires ga:gitRepository or ga:rdfDocument",
            configuration.id()
        )
    })?;
    let reference = configuration.text("gitRef")?;
    ensure!(
        reference.len() <= 1024 && !reference.chars().any(char::is_control),
        "family Git ref must be a nonempty bounded reference"
    );
    let hints = configuration
        .optional_text("hintsPath")?
        .map(std::path::PathBuf::from);
    let (built, additional) = match local {
        None => {
            let selection = hinted_selection(configuration)?;
            let repository = super::collect::git::remote_publication(
                &url,
                &reference,
                cache,
                hints.as_deref(),
                &selection,
            )
            .await?;
            (
                super::ontology_family::collect_prepared(
                    Dataset::memory()?,
                    repository,
                    4,
                )
                .await?,
                Vec::new(),
            )
        },
        Some(local) => {
            ensure!(
                hints.is_none(),
                "explicit local definitions conflict with repository hints \
                 path"
            );
            (
                super::ontology_family::collect_remote(
                    &url,
                    &reference,
                    cache,
                    &local.path,
                    &local.project,
                    &local.target,
                    4,
                )
                .await?,
                local.additional,
            )
        },
    };
    family_targets(built, &additional).await
}

async fn family_targets(
    built: super::ontology_family::CollectedFamily,
    additional: &[String],
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    anyhow::ensure!(
        additional.len() <= 8,
        "family showcase exceeds eight additional targets"
    );
    let view =
        RevisionView::new(built.dataset.clone(), built.plan.snapshot.clone());
    let project = view.node(&built.plan.id, "project").await?;
    let revision = view.node(&built.plan.id, "inputRevision").await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 8, 4 * 1024 * 1024)?;
    // bounded: at most sixteen selected targets plus eight explicit targets.
    let mut plans = built.plans;
    let result = async {
        let planner = TargetPlanner::new(built.dataset.clone(), revision);
        for target in additional {
            let plan = planner
                .resolve(
                    project.as_str(),
                    &NamedNode::new(target)?,
                    &writer,
                )
                .await?;
            anyhow::ensure!(
                !plans.iter().any(|existing| existing.id == plan.id),
                "duplicate family showcase target"
            );
            plans.push(plan);
        }
        anyhow::Ok(())
    }
    .await;
    drop(writer);
    worker.await??;
    result?;
    Ok((built.dataset, plans))
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
    let configuration =
        ShowcaseConfiguration::read(&sources.join("showcase.ttl")).await?;
    configuration.accept_only("W3C specification", &[])?;
    let document = tokio::fs::canonicalize(sources.join("source.html")).await?;
    let copied = configuration.copied_sources()?;
    let source = copied
        .iter()
        .find(|copied| copied.path() == document)
        .and_then(|copied| copied.origin())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} must record the published document source.html was copied \
                 from",
                configuration.path().display()
            )
        })?;
    let project = "urn:w3c-example:project";
    let collector = W3cSpecificationFile::new(
        project,
        "urn:w3c-example:collector",
        document,
        source.as_str(),
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

/// Build an exact-revision showcase: definitions from a controlling commit,
/// content from a designated commit of the same repository, deployed at
/// `publication_url`.
pub async fn revision_build(
    source: &Path,
    cache: &Path,
    publication_url: super::revision_build::PublicationUrl,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    super::showcases_revision::build(
        &super::showcase_configuration::ShowcaseConfiguration::read(source)
            .await?,
        cache,
        publication_url,
    )
    .await
}

/// Collect and plan the standalone native SKOS showcase.
pub async fn skos(source: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    super::showcases_semantic::skos(source).await
}
/// Collect actual EKGF UseCase instances and generate their documentation.
pub async fn use_cases(source: &Path) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    super::showcases_semantic::use_cases(source).await
}
