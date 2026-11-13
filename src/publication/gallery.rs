//! Build declared showcase fixtures and collect their published artifacts into
//! the documentation KG. Expected-error fixtures are explicit catalog entries.
use {
    super::{
        RDF_TYPE,
        collect::{
            Collector,
            RdfFile,
            collect_all,
            published_artifact::PublishedArtifactFile,
        },
        dataset::Dataset,
        definitions::{objects, text},
        diagnostics::FailureMode,
        render::Publisher,
        revision_build::PublicationUrl,
        showcase_configuration::ShowcaseConfiguration,
        showcases,
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Context, Result, ensure},
    oxigraph::{io::RdfFormat, store::Store},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    runner::ShowcaseRunner,
    std::{
        collections::{BTreeSet, HashMap},
        path::{Component, Path},
        sync::Arc,
    },
    tokio::io::AsyncReadExt,
};

struct Case {
    id:          NamedNode,
    key:         String,
    runner:      ShowcaseRunner,
    source:      String,
    preview:     Option<String>,
    pdf:         Option<String>,
    expected:    BTreeSet<String>,
    /// Deployed location of this case's artifacts, supplied to an
    /// exact-revision build by the gallery rather than by its definitions.
    publication: Option<PublicationUrl>,
}

/// Complete, bounded coverage of the declared showcase suite.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GalleryReport {
    pub schema_version:  u32,
    pub succeeded:       bool,
    pub cases:           Vec<ShowcaseReport>,
    pub dataset_archive: Option<std::path::PathBuf>,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ShowcaseReport {
    pub id:              String,
    pub source:          String,
    pub succeeded:       bool,
    pub error:           Option<String>,
    pub dataset_archive: Option<std::path::PathBuf>,
}
impl GalleryReport {
    pub fn ensure_success(&self) -> Result<()> {
        let failures = self
            .cases
            .iter()
            .filter_map(|case| {
                case.error.as_ref().map(|error| {
                    format!("{} [{}]: {error}", case.id, case.source)
                })
            })
            .collect::<Vec<_>>();
        ensure!(
            self.succeeded,
            "showcase verification failed:\n{}",
            failures.join("\n")
        );
        Ok(())
    }
}

pub(crate) async fn collect(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
) -> Result<()> {
    let stage = tempfile::tempdir()?;
    let report =
        collect_at(root, dataset, writer, project, stage.path()).await?;
    report.ensure_success()
}

/// Retain per-case evidence even when one or more live showcases fail.
pub async fn verify(
    dataset: Dataset,
    root: &Path,
    project: &str,
    output: &Path,
) -> Result<GalleryReport> {
    tokio::fs::create_dir_all(output).await?;
    let output = tokio::fs::canonicalize(output).await?;
    ensure!(
        std::fs::read_dir(&output)?.next().transpose()?.is_none(),
        "gallery verification output must be new or empty; stale artifacts \
         are not reused"
    );
    let root = tokio::fs::canonicalize(root).await?;
    let stage = output.join("artifacts");
    tokio::fs::create_dir(&stage).await?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
    let result = collect_at(&root, &dataset, &writer, project, &stage).await;
    drop(writer);
    worker.await??;
    let mut report = result?;
    let archive = output.join("verification.nq");
    dataset.export(archive.clone()).await?;
    report.dataset_archive = Some(archive);
    tokio::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )
    .await?;
    Ok(report)
}

async fn collect_at(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    stage_root: &Path,
) -> Result<GalleryReport> {
    let registry = root.join("showcases.ttl");
    let mut file = tokio::fs::File::open(&registry).await?.take(256 * 1024 + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).await?;
    ensure!(
        bytes.len() <= 256 * 1024,
        "showcase registry exceeds byte budget"
    );
    let project_id = NamedNode::new(project)?;
    let cases = tokio::task::spawn_blocking(move || read(&bytes, &project_id))
        .await??;
    RdfFile::new(
        project,
        "urn:grapharch:showcase-registry",
        registry,
    )?
    .collect(writer)
    .await?;
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    let mut metadata = Vec::new();
    let mut total_bytes = 0u64;
    let build_started = std::time::Instant::now();
    let observed = dataset.snapshot_for(project).await?;
    let mut reports = Vec::new();
    use futures::StreamExt;
    let mut pending = futures::stream::iter(cases)
        .map(|case| {
            async move {
                let id = case.id.as_str().to_owned();
                let source = case.source.clone();
                let key = case.key.clone();
                (
                    id,
                    source,
                    key,
                    build_case(case, root, stage_root, project).await,
                )
            }
        })
        .buffer_unordered(2);
    while let Some((id, source, key, result)) = pending.next().await {
        let archive = stage_root.join(&key).join("documentation.nq");
        let mut report = ShowcaseReport {
            id,
            source,
            succeeded: false,
            error: None,
            dataset_archive: archive.is_file().then_some(archive),
        };
        match result {
            Ok(built) => {
                if total_bytes + built.bytes >
                    super::limits::RETAINED_ASSET_BYTES
                {
                    report.error = Some(
                        "showcase artifacts exceed 512 MiB gallery budget"
                            .into(),
                    );
                } else {
                    total_bytes += built.bytes;
                    work.extend(built.work);
                    metadata.extend(built.metadata);
                    report.succeeded = true;
                }
            },
            Err(error) => report.error = Some(format!("{error:#}")),
        }
        reports.push(report);
    }
    // Every case has completed before errors can propagate from reporting or
    // aggregation.
    reports.sort_by(|left, right| left.id.cmp(&right.id));
    for report in &reports {
        if let Some(error) = &report.error {
            super::diagnostics::record(
                dataset,
                &super::diagnostics::SourceContext {
                    project,
                    processor: &report.id,
                    source: &report.source,
                    stage: "showcase-verification",
                    input_revision: Some(&observed),
                },
                super::diagnostics::Diagnostic {
                    severity:        super::diagnostics::Severity::Error,
                    code:            "showcase-build-failed".into(),
                    message:         error.clone(),
                    reference:       report.id.clone(),
                    source_fragment: None,
                },
            )
            .await?;
        }
    }
    validate_artifact_count(work.len())?;
    tracing::debug!(
        elapsed_ms = build_started.elapsed().as_millis(),
        "all showcase artifacts built"
    );
    collect_all(work, writer, 4).await?;
    tracing::debug!(
        elapsed_ms = build_started.elapsed().as_millis(),
        "showcase artifacts collected"
    );
    let snapshot = dataset.snapshot_for(project).await?;
    let mut stream = writer
        .begin(
            project,
            "urn:grapharch:gallery-bindings",
            "urn:grapharch:gallery-bindings",
            &format!("gallery-bindings-v2:{snapshot}"),
        )
        .await?;
    for quad in metadata {
        stream.write(quad).await?;
    }
    stream.complete().await?;
    Ok(GalleryReport {
        schema_version:  1,
        succeeded:       reports.iter().all(|case| case.succeeded),
        cases:           reports,
        dataset_archive: None,
    })
}

struct BuiltCase {
    work:     Vec<Arc<dyn Collector>>,
    metadata: Vec<Quad>,
    bytes:    u64,
}

async fn build_case(
    case: Case,
    root: &Path,
    stage: &Path,
    project: &str,
) -> Result<BuiltCase> {
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    let mut metadata = Vec::new();
    let mut total_bytes = 0u64;
    let case_started = std::time::Instant::now();
    let source = tokio::fs::canonicalize(root.join(&case.source)).await?;
    ensure!(
        source.starts_with(root),
        "showcase source escapes its declared root"
    );
    let cache = root
        .parent()
        .ok_or_else(|| {
            anyhow::anyhow!("showcase root requires a project directory")
        })?
        .join("output/cache/git");
    let (child, plans) = match case.runner {
        ShowcaseRunner::Authored => showcases::authored(&source).await?,
        ShowcaseRunner::RevisionBuild => {
            super::showcases_revision::build(
                &configuration(&case, &source).await?,
                &cache,
                case.publication.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "revision-build showcase {} has no publication URL",
                        case.id
                    )
                })?,
            )
            .await?
        },
        ShowcaseRunner::Mkdocs => {
            super::showcases_mkdocs::configured(
                &configuration(&case, &source).await?,
                &cache,
            )
            .await?
        },
        ShowcaseRunner::OntologyFamily => {
            showcases::family_configured(
                &configuration(&case, &source).await?,
                &cache,
            )
            .await?
        },
        ShowcaseRunner::Skos => showcases::skos(&source).await?,
        ShowcaseRunner::UseCases => showcases::use_cases(&source).await?,
        ShowcaseRunner::MixedSources => showcases::mixed(&source).await?,
        ShowcaseRunner::Specification => {
            showcases::specification(&source).await?
        },
        ShowcaseRunner::W3cSpecification => showcases::w3c(&source).await?,
    };
    let mode = if case.expected.is_empty() {
        FailureMode::FailFast
    } else {
        FailureMode::KeepGoing
    };
    let output = stage.join(&case.key);
    tokio::fs::create_dir_all(&output).await?;
    let output = tokio::fs::canonicalize(output).await?;
    let publications = showcases::publish(
        &Publisher::new(child.clone())
            .with_edition(crate::publication::pdf_name::Edition::Default)
            .with_failure_mode(mode),
        &plans,
        &output,
    )
    .await;
    // Archive the observed KG after the publication attempt, including any
    // diagnostics.
    let archive = output.join("documentation.nq");
    let temporary = output.join("documentation.nq.partial");
    let archived = async {
        child.export(temporary.clone()).await?;
        tokio::fs::rename(&temporary, &archive).await?;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let publications = match (publications, archived) {
        (Err(error), Err(archive_error)) => {
            anyhow::bail!(
                "{error:#}; could not archive diagnostics: {archive_error:#}"
            )
        },
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(error),
        (Ok(publications), Ok(())) => publications,
    };
    let actual: Vec<_> = publications
        .iter()
        .flat_map(|publication| {
            publication
                .manifest
                .diagnostics
                .iter()
                .filter(|d| {
                    matches!(d.severity, super::diagnostics::Severity::Error)
                })
                .cloned()
        })
        .collect();
    let codes: BTreeSet<_> =
        actual.iter().map(|error| error.code.clone()).collect();
    ensure!(
        codes == case.expected && actual.len() == case.expected.len(),
        "showcase {} produced unexpected diagnostics: expected {:?}, observed \
         {:?} ({} diagnostics)",
        case.id,
        case.expected,
        codes,
        actual.len()
    );
    let prefix = format!("showcases/artifacts/{}/", case.key);
    let mut artifacts = HashMap::new();
    let mut output_order = 0u32;
    // bounded: one named PDF edition per publication of this case.
    let mut pdfs = std::collections::HashSet::new();
    for publication in publications {
        let target = NamedNode::new(&publication.manifest.target)?;
        let view = RevisionView::new(
            child.clone(),
            NamedNode::new(&publication.manifest.input_revision)?,
        );
        let title = super::render::page_title(&view, &target).await?;
        let mut files = publication.manifest.files.clone();
        if let Some(placement) =
            super::graph_placement::GraphPlacement::of(&view, &target).await?
        {
            files.push(placement.retain(&publication, &archive).await?);
        }
        let pdf_name = publication.manifest.pdf.clone();
        for name in files {
            let file = publication.directory.join(&name);
            let relative = file
                .strip_prefix(&output)?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF-8 showcase path"))?
                .replace('\\', "/");
            let asset = PublishedArtifactFile::new(
                project,
                file.clone(),
                format!("{prefix}{relative}"),
            )?;
            total_bytes += tokio::fs::metadata(&file).await?.len();
            ensure!(
                total_bytes <= super::limits::RETAINED_ASSET_BYTES,
                "showcase artifacts exceed 512 MiB budget"
            );
            let is_pdf = pdf_name.as_deref() == Some(name.as_str());
            if is_pdf {
                pdfs.insert(relative.clone());
            }
            artifacts.insert(relative, asset.id());
            if name == "index.html" || is_pdf {
                metadata.push(quad(&case.id, "outputArtifact", asset.id()));
                metadata.push(quad(
                    &asset.id(),
                    "order",
                    Literal::from(output_order),
                ));
                output_order += 1;
                metadata.push(Quad::new(
                    asset.id(),
                    NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                    Literal::new_simple_literal(format!(
                        "{title} ({})",
                        if name.ends_with("pdf") { "PDF" } else { "HTML" }
                    )),
                    GraphName::DefaultGraph,
                ));
            }
            work.push(Arc::new(asset));
        }
    }
    links::PublishedOutput::new(
        output.clone(),
        artifacts
            .keys()
            .cloned()
            .chain(["documentation.nq".to_owned()])
            .collect(),
    )
    .verify_links()
    .await
    .with_context(|| format!("showcase {} publishes broken links", case.id))?;
    let preview = artifact_selection(
        &artifacts,
        case.preview.as_deref(),
        |path| {
            Path::new(path)
                .file_name()
                .is_some_and(|name| name == "index.html")
        },
        "index.html",
        &case.id,
    )?
    .ok_or_else(|| {
        anyhow::anyhow!("showcase {} has no HTML preview", case.id)
    })?;
    let pdf = artifact_selection(
        &artifacts,
        case.pdf.as_deref(),
        |path| pdfs.contains(path),
        "PDF edition",
        &case.id,
    )?;
    if let Some(pdf) = pdf {
        metadata.push(quad(&case.id, "pdfArtifact", pdf.clone()));
    }
    metadata.extend([
        quad(&case.id, "previewArtifact", preview.clone()),
        quad(
            &case.id,
            "diagnosticCount",
            Literal::from(actual.len() as i64),
        ),
    ]);
    let archive = output.join("documentation.nq");
    total_bytes += tokio::fs::metadata(&archive).await?.len();
    ensure!(
        total_bytes <= super::limits::RETAINED_ASSET_BYTES,
        "showcase artifacts exceed 512 MiB budget"
    );
    let archive = PublishedArtifactFile::new(
        project,
        archive,
        format!("{prefix}documentation.nq"),
    )?;
    metadata.push(quad(&case.id, "datasetArtifact", archive.id()));
    work.push(Arc::new(archive));
    tracing::debug!(case = %case.id, elapsed_ms = case_started.elapsed().as_millis(), "showcase built");

    Ok(BuiltCase { work, metadata, bytes: total_bytes })
}

fn artifact_selection<'a>(
    artifacts: &'a HashMap<String, NamedNode>,
    selected: Option<&str>,
    candidate: impl Fn(&str) -> bool,
    filename: &str,
    case: &NamedNode,
) -> Result<Option<&'a NamedNode>> {
    if let Some(path) = selected {
        return artifacts.get(path).map(Some).ok_or_else(|| {
            anyhow::anyhow!("showcase {case} has no selected artifact {path}")
        });
    }
    let mut choices = artifacts.iter().filter(|(path, _)| candidate(path));
    let Some((_, asset)) = choices.next() else {
        return Ok(None);
    };
    ensure!(
        choices.next().is_none(),
        "showcase {case} requires explicit selection among multiple \
         {filename} artifacts"
    );
    Ok(Some(asset))
}

fn optional_text(
    store: &Store,
    subject: &NamedNode,
    field: &str,
) -> Result<Option<String>> {
    let mut values = objects(store, subject, field)?;
    let value = values.next().transpose()?;
    ensure!(
        values.next().is_none(),
        "showcase {field} must have at most one value"
    );
    match value {
        None => Ok(None),
        Some(Term::Literal(value)) => Ok(Some(value.value().to_owned())),
        _ => anyhow::bail!("showcase {field} must be a literal"),
    }
}

fn quad(subject: &NamedNode, field: &str, object: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        term(field),
        object,
        GraphName::DefaultGraph,
    )
}

fn read(bytes: &[u8], project: &NamedNode) -> Result<Vec<Case>> {
    let store = Store::new()?;
    store.load_from_reader(RdfFormat::Turtle, bytes)?;
    let artifacts = artifacts_url(&store, project)?;
    let mut cases = Vec::new();
    let mut keys = BTreeSet::new();
    for object in objects(&store, project, "showcase")? {
        let Term::NamedNode(id) = object? else {
            anyhow::bail!("showcase must be an IRI")
        };
        ensure!(
            store.contains(
                Quad::new(
                    id.clone(),
                    NamedNode::new_unchecked(RDF_TYPE),
                    term("Showcase"),
                    GraphName::DefaultGraph
                )
                .as_ref()
            )?,
            "showcase must be declared"
        );
        let source = text(&store, &id, "source")?;
        let key = text(&store, &id, "slug")?;
        ensure!(
            !key.is_empty() &&
                key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid showcase slug"
        );
        ensure!(
            keys.insert(key.clone()),
            "duplicate showcase identity"
        );
        let preview = optional_text(&store, &id, "previewPath")?;
        let pdf = optional_text(&store, &id, "pdfPath")?;
        for path in std::iter::once(source.as_str())
            .chain(preview.as_deref())
            .chain(pdf.as_deref())
        {
            ensure!(
                !path.is_empty() &&
                    Path::new(path)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                "invalid showcase path {path}"
            );
        }
        let expected = objects(&store, &id, "expectedDiagnostic")?
            .map(|term| {
                match term? {
                    Term::Literal(code) => Ok(code.value().to_owned()),
                    _ => anyhow::bail!("expected diagnostic must be a literal"),
                }
            })
            .collect::<Result<_>>()?;
        let runner = runner(&store, &id)?;
        let publication = match (runner, &artifacts) {
            (ShowcaseRunner::RevisionBuild, Some(artifacts)) => {
                Some(PublicationUrl::try_new(&format!(
                    "{}{key}/",
                    artifacts.as_str()
                ))?)
            },
            (ShowcaseRunner::RevisionBuild, None) => {
                anyhow::bail!(
                    "{id} is an exact-revision showcase; {project} requires \
                     ga:showcaseArtifactsUrl, the deployed location its \
                     publication URL derives from"
                )
            },
            _ => None,
        };
        cases.push(Case {
            runner,
            id,
            key,
            source,
            preview,
            pdf,
            expected,
            publication,
        });
        ensure!(
            cases.len() <= 16,
            "gallery exceeds 16-showcase budget"
        );
    }
    ensure!(!cases.is_empty(), "showcase registry is empty");
    Ok(cases)
}

/// The run configuration a registered showcase names with `ga:source`; it
/// must describe that same showcase.
async fn configuration(
    case: &Case,
    source: &Path,
) -> Result<ShowcaseConfiguration> {
    let configuration = ShowcaseConfiguration::read(source).await?;
    ensure!(
        configuration.id() == &case.id,
        "{} configures {}, not the registered showcase {}",
        case.source,
        configuration.id(),
        case.id
    );
    Ok(configuration)
}

/// The deployed directory under which every showcase publishes its
/// artifacts at its slug.
fn artifacts_url(
    store: &Store,
    project: &NamedNode,
) -> Result<Option<PublicationUrl>> {
    let mut values = objects(store, project, "showcaseArtifactsUrl")?;
    let Some(value) = values.next().transpose()? else {
        return Ok(None);
    };
    ensure!(
        values.next().transpose()?.is_none(),
        "{project} has conflicting showcaseArtifactsUrl values"
    );
    let Term::NamedNode(url) = value else {
        anyhow::bail!("{project} showcaseArtifactsUrl must be an IRI")
    };
    Ok(Some(PublicationUrl::try_new(url.as_str())?))
}

fn validate_artifact_count(count: usize) -> Result<()> {
    // Bounded: at most 16 registered cases can retain up to 512 aggregate
    // outputs, including ontology library pages and multiple PDF editions.
    // The independent 512 MiB retained-byte budget also remains enforced.
    ensure!(
        count <= super::limits::GALLERY_ARTIFACTS,
        "showcase gallery exceeds {}-artifact budget",
        super::limits::GALLERY_ARTIFACTS
    );
    Ok(())
}

fn runner(store: &Store, id: &NamedNode) -> Result<ShowcaseRunner> {
    let mut runners = objects(store, id, "showcaseRunner")?;
    let Some(runner) = runners.next().transpose()? else {
        anyhow::bail!("{id} missing showcaseRunner")
    };
    ensure!(
        runners.next().transpose()?.is_none(),
        "{id} has conflicting showcaseRunner values"
    );
    let Term::NamedNode(runner) = runner else {
        anyhow::bail!("{id} showcaseRunner must be an IRI")
    };
    ShowcaseRunner::from_node(&runner)
}

#[path = "gallery_links.rs"]
mod links;
#[path = "gallery_runner.rs"]
mod runner;

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
