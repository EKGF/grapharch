//! Independently invocable operations for the graph-backed publication
//! pipeline.

use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            MarkdownFile,
            RdfFile,
            SparqlFile,
            W3cSpecificationFile,
        },
        dataset::Dataset,
        diagnostics::FailureMode,
        plan::{ResolvedPlan, TargetPlanner},
        render::{PublishOutcome, Publisher},
        scope::ScopeResolver,
        validate,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    clap::{Args, Subcommand, ValueEnum},
    oxigraph::{io::RdfFormat, store::Store},
    oxrdf::{NamedNode, Term},
    std::path::PathBuf,
    tokio::io::AsyncReadExt,
};

/// Exactly one store location is selected for every independent pipeline stage.
#[derive(Args)]
pub struct StoreOptions {
    #[arg(
        long,
        required_unless_present = "sparql_store",
        conflicts_with = "sparql_store"
    )]
    store:        Option<PathBuf>,
    /// YAML configuration for a transaction-capable external SPARQL KG.
    #[arg(long)]
    sparql_store: Option<PathBuf>,
}
impl StoreOptions {
    async fn open(&self) -> Result<Dataset> {
        if let Some(path) = &self.sparql_store {
            let bytes = bounded_file(path, 64 * 1024).await?;
            Dataset::sparql(serde_yaml::from_slice(&bytes)?).await
        } else {
            Dataset::open(
                self.store
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("a store is required"))?,
            )
            .await
        }
    }
}

#[derive(Args)]
pub struct BuildStoreOptions {
    #[arg(long, conflicts_with = "sparql_store")]
    store:        Option<PathBuf>,
    #[arg(long)]
    sparql_store: Option<PathBuf>,
}
impl BuildStoreOptions {
    async fn open(&self) -> Result<Dataset> {
        match (&self.store, &self.sparql_store) {
            (Some(path), None) => Dataset::open(path.clone()).await,
            (None, Some(path)) => {
                Dataset::sparql(serde_yaml::from_slice(
                    &bounded_file(path, 64 * 1024).await?,
                )?)
                .await
            },
            (None, None) => Dataset::memory(),
            _ => anyhow::bail!("choose one KG backend"),
        }
    }
}

/// Independently executable publication operations using a persistent shared
/// dataset.
#[derive(Subcommand)]
pub enum PublicationCommand {
    /// Collect a documentation tree and publish its logical topics as a
    /// website.
    BuildSite {
        #[arg(long)]
        source:      PathBuf,
        #[arg(long)]
        definitions: PathBuf,
        #[arg(long)]
        project:     String,
        #[arg(long)]
        target:      String,
        #[arg(long)]
        output_root: PathBuf,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
    /// Collect an explicitly registered family of RDF ontologies and publish
    /// its reference.
    BuildFamily {
        #[command(flatten)]
        backend:          BuildStoreOptions,
        /// YAML list of stable collector identities and endpoint graph
        /// queries.
        #[arg(long,conflicts_with_all=["source","local_repository","github","git_repository"])]
        sparql_sources:   Option<PathBuf>,
        #[arg(long, required_unless_present_any = ["local_repository", "github", "git_repository", "sparql_sources"], conflicts_with_all = ["local_repository", "github", "git_repository"])]
        source:           Option<PathBuf>,
        /// Read a local Git clone's working tree directly.
        #[arg(long, conflicts_with_all = ["github", "git_repository"])]
        local_repository: Option<PathBuf>,
        /// Read OWNER/REPO on GitHub through a bare Git cache.
        #[arg(long, conflicts_with = "git_repository")]
        github:           Option<String>,
        /// Read any remote Git URL through a bare Git cache.
        #[arg(long)]
        git_repository:   Option<String>,
        /// Branch, full tag ref, HEAD, or commit ID for a remote source.
        #[arg(long)]
        git_ref:          Option<String>,
        /// Bare-cache parent directory (defaults to the system temp cache).
        #[arg(long)]
        git_cache:        Option<PathBuf>,
        #[arg(long)]
        definitions:      PathBuf,
        #[arg(long)]
        project:          String,
        #[arg(long)]
        target:           String,
        #[arg(long)]
        output_root:      PathBuf,
        #[arg(long, default_value_t = 4)]
        concurrency:      usize,
    },
    /// Render a pinned target plan without rereading its original sources.
    Render {
        #[command(flatten)]
        store:       StoreOptions,
        #[arg(long)]
        plan:        String,
        #[arg(long)]
        snapshot:    String,
        #[arg(long)]
        output_root: PathBuf,
    },
    /// Assemble and atomically promote a validated project revision.
    Assemble {
        #[command(flatten)]
        store:   StoreOptions,
        #[arg(long)]
        project: String,
    },
    /// Resolve one target or every registered target from a validated revision.
    Plan {
        #[command(flatten)]
        store:    StoreOptions,
        #[arg(long)]
        project:  String,
        #[arg(long)]
        revision: String,
        #[arg(long, required_unless_present = "all", conflicts_with = "all")]
        target:   Option<String>,
        #[arg(long)]
        all:      bool,
    },
    /// Validate a bounded standalone Turtle definition document.
    Validate {
        #[arg(long)]
        definitions: PathBuf,
    },
    /// Collect one source through a bounded writer into the selected KG.
    Collect {
        #[command(flatten)]
        store:      StoreOptions,
        #[arg(long)]
        project:    String,
        #[arg(long)]
        collector:  String,
        #[arg(long, value_enum)]
        kind:       SourceKind,
        #[arg(long)]
        source:     PathBuf,
        /// Original publication URL, required only for w3c-specification.
        #[arg(long)]
        source_url: Option<String>,
    },
    /// Pin current complete source contributions without copying their
    /// assertions.
    Snapshot {
        #[command(flatten)]
        store: StoreOptions,
    },
    /// Resolve a scope against an explicit pinned snapshot.
    ResolveScope {
        #[command(flatten)]
        store:    StoreOptions,
        #[arg(long)]
        project:  String,
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        scope:    String,
    },
    /// Stream an explicit inspection SELECT query from a UTF-8 query file.
    Query {
        #[command(flatten)]
        store: StoreOptions,
        #[arg(long)]
        query: PathBuf,
    },
    /// Export a complete dataset, including contributions and pinned snapshots.
    Export {
        #[command(flatten)]
        store:  StoreOptions,
        #[arg(long)]
        output: PathBuf,
    },
    /// Import into an empty dataset; existing data is never merged implicitly.
    Import {
        #[command(flatten)]
        store: StoreOptions,
        #[arg(long)]
        input: PathBuf,
    },
}

/// Supported narrowly specialized source adapters.
#[derive(Clone, Copy, ValueEnum)]
pub enum SourceKind {
    Markdown,
    Sparql,
    Rdf,
    SparqlEndpoint,
    W3cSpecification,
}

/// Dispatch one operation. No source access is required for
/// snapshot/query/resolve/export.
pub async fn run(
    command: &PublicationCommand,
    mode: FailureMode,
) -> Result<()> {
    match command {
        PublicationCommand::BuildFamily {
            backend,
            sparql_sources,
            source,
            local_repository,
            github,
            git_repository,
            git_ref,
            git_cache,
            definitions,
            project,
            target,
            output_root,
            concurrency,
        } => {
            ensure!(*concurrency > 0, "concurrency must be positive");
            let dataset = backend.open().await?;
            let built = if let Some(path) = sparql_sources {
                ensure!(
                    git_ref.is_none() && git_cache.is_none(),
                    "Git options require a Git source"
                );
                let configs = serde_yaml::from_slice::<
                    Vec<super::ontology_family::SparqlFamilyCollector>,
                >(
                    &bounded_file(path, 256 * 1024).await?
                )?;
                super::ontology_family::collect_from(
                    dataset,
                    super::ontology_family::FamilySource::Sparql(&configs),
                    definitions,
                    project,
                    target,
                    *concurrency,
                )
                .await?
            } else if let Some(source) = source {
                ensure!(
                    git_ref.is_none() && git_cache.is_none(),
                    "Git options require a remote repository"
                );
                super::ontology_family::collect_from(
                    dataset,
                    super::ontology_family::FamilySource::Directory(source),
                    definitions,
                    project,
                    target,
                    *concurrency,
                )
                .await?
            } else if let Some(source) = local_repository {
                ensure!(
                    git_ref.is_none() && git_cache.is_none(),
                    "working-tree mode does not accept remote Git options"
                );
                super::ontology_family::collect_from(
                    dataset,
                    super::ontology_family::FamilySource::LocalRepository(
                        source,
                    ),
                    definitions,
                    project,
                    target,
                    *concurrency,
                )
                .await?
            } else {
                let url = if let Some(repository) = github {
                    let parts: Vec<_> = repository.split('/').collect();
                    ensure!(
                        parts.len() == 2 &&
                            parts.iter().all(|p| {
                                !p.is_empty() &&
                                    p.chars().all(|c| {
                                        c.is_ascii_alphanumeric() ||
                                            matches!(c, '-' | '_' | '.')
                                    })
                            }) &&
                            parts.iter().all(|p| !matches!(*p, "." | "..")),
                        "GitHub repository must be OWNER/REPO"
                    );
                    format!("https://github.com/{repository}.git")
                } else {
                    git_repository
                        .as_ref()
                        .ok_or_else(|| {
                            anyhow::anyhow!("repository source required")
                        })?
                        .clone()
                };
                let reference = git_ref.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("remote repository requires --git-ref")
                })?;
                let cache = git_cache.clone().unwrap_or_else(|| {
                    std::env::temp_dir().join("grapharch-git-cache")
                });
                super::ontology_family::collect_from(
                    dataset,
                    super::ontology_family::FamilySource::RemoteRepository {
                        url: &url,
                        reference,
                        cache: &cache,
                    },
                    definitions,
                    project,
                    target,
                    *concurrency,
                )
                .await?
            };
            let outcome = Publisher::new(built.dataset.clone())
                .with_failure_mode(mode)
                .render(&built.plan, output_root)
                .await?;
            let PublishOutcome::Published(publication) = outcome else {
                anyhow::bail!("family publication was superseded")
            };
            built
                .dataset
                .export(output_root.join("documentation.nq"))
                .await?;
            println!(
                "{}\nontologies={}\nresources={}\nplan={}\nsnapshot={}",
                publication.directory.display(),
                built.ontology_count,
                built.term_count,
                built.plan.id,
                built.plan.snapshot
            );
        },
        PublicationCommand::BuildSite {
            source,
            definitions,
            project,
            target,
            output_root,
            concurrency,
        } => {
            let built = super::website::collect(
                source,
                definitions,
                project,
                target,
                *concurrency,
            )
            .await?;
            match Publisher::new(built.dataset.clone())
                .with_failure_mode(mode)
                .render(&built.plan, output_root)
                .await?
            {
                PublishOutcome::Published(publication) => {
                    println!("{}", publication.directory.display())
                },
                PublishOutcome::Superseded => {
                    anyhow::bail!("website publication superseded")
                },
            }
            built
                .dataset
                .export(output_root.join("documentation.nq"))
                .await?;
            println!(
                "plan={}\nsnapshot={}",
                built.plan.id, built.plan.snapshot
            );
        },
        PublicationCommand::Render { store, plan, snapshot, output_root } => {
            let dataset = store.open().await?;
            let plan = ResolvedPlan {
                id:       NamedNode::new(plan)?,
                snapshot: NamedNode::new(snapshot)?,
            };
            match Publisher::new(dataset)
                .with_failure_mode(mode)
                .render(&plan, output_root)
                .await?
            {
                PublishOutcome::Published(publication) => {
                    println!("{}", publication.directory.display());
                    for diagnostic in &publication.manifest.diagnostics {
                        eprintln!(
                            "{}: {}",
                            diagnostic.code, diagnostic.message
                        );
                    }
                    ensure!(
                        publication.manifest.diagnostics.is_empty(),
                        "publication produced with {} error(s)",
                        publication.manifest.diagnostics.len()
                    );
                },
                PublishOutcome::Superseded => println!("superseded"),
            }
        },
        PublicationCommand::Assemble { store, project } => {
            let dataset = store.open().await?;
            println!(
                "{}",
                Assembler::new(dataset).assemble(project).await?.as_str()
            );
        },
        PublicationCommand::Plan {
            store,
            project,
            revision,
            target,
            all: _,
        } => {
            let dataset = store.open().await?;
            let revision = NamedNode::new(revision)?;
            Assembler::new(dataset.clone())
                .require_validated(project, &revision)
                .await?;
            let planner = TargetPlanner::new(dataset.clone(), revision.clone());
            let (writer, task) =
                GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
            let result = async {
                if let Some(target) = target {
                    let plan = planner
                        .resolve(project, &NamedNode::new(target)?, &writer)
                        .await?;
                    println!(
                        "plan={}\nsnapshot={}",
                        plan.id.as_str(),
                        plan.snapshot.as_str()
                    );
                } else {
                    let view = RevisionView::new(dataset, revision);
                    let mut targets = view
                        .objects(&NamedNode::new(project)?, "target")
                        .await?;
                    while let Some(row) = targets.next().await? {
                        let Term::NamedNode(target) = &row["value"] else {
                            anyhow::bail!("target must be an IRI")
                        };
                        let plan =
                            planner.resolve(project, target, &writer).await?;
                        println!(
                            "plan={}\nsnapshot={}",
                            plan.id.as_str(),
                            plan.snapshot.as_str()
                        );
                    }
                }
                anyhow::Ok(())
            }
            .await;
            drop(writer);
            task.await??;
            result?;
        },
        PublicationCommand::Validate { definitions } => {
            let bytes = bounded_file(definitions, 256 * 1024).await?;
            tokio::task::spawn_blocking(move || -> Result<()> {
                let store = Store::new()?;
                // bounded: standalone definitions are capped at 256 KiB, not a
                // corpus.
                store.load_from_reader(RdfFormat::Turtle, bytes.as_slice())?;
                validate(&store)
            })
            .await??;
            println!("Definitions valid");
        },
        PublicationCommand::Collect {
            store,
            project,
            collector,
            kind,
            source,
            source_url,
        } => {
            ensure!(
                matches!(kind, SourceKind::W3cSpecification) ==
                    source_url.is_some(),
                "--source-url is required for w3c-specification and is \
                 invalid for other collector kinds"
            );
            let dataset = store.open().await?;
            let (writer, task) =
                GraphWriter::spawn(dataset, 8, 4 * 1024 * 1024)?;
            let result = match kind {
                SourceKind::W3cSpecification => {
                    W3cSpecificationFile::new(
                        project,
                        collector,
                        source.clone(),
                        source_url.as_deref().ok_or_else(|| {
                            anyhow::anyhow!("missing --source-url")
                        })?,
                    )?
                    .collect(&writer)
                    .await
                },
                SourceKind::Markdown => {
                    MarkdownFile::new(project, collector, source.clone())?
                        .collect(&writer)
                        .await
                },
                SourceKind::Sparql => {
                    SparqlFile::new(project, collector, source.clone())?
                        .collect(&writer)
                        .await
                },
                SourceKind::SparqlEndpoint => {
                    let config = serde_yaml::from_slice(
                        &bounded_file(source, 128 * 1024).await?,
                    )?;
                    super::collect::SparqlEndpointSource::new(
                        project, collector, config,
                    )?
                    .collect(&writer)
                    .await
                },
                SourceKind::Rdf => {
                    RdfFile::new(project, collector, source.clone())?
                        .collect(&writer)
                        .await
                },
            };
            drop(writer);
            task.await??;
            result?;
        },
        PublicationCommand::Snapshot { store } => {
            let dataset = store.open().await?;
            println!("{}", dataset.snapshot().await?.as_str());
        },
        PublicationCommand::ResolveScope {
            store,
            project,
            snapshot,
            scope,
        } => {
            let dataset = store.open().await?;
            dataset.ensure_ready(project).await?;
            let resolver =
                ScopeResolver::new(dataset.clone(), NamedNode::new(snapshot)?);
            let (writer, task) =
                GraphWriter::spawn(dataset, 8, 4 * 1024 * 1024)?;
            let result = resolver
                .resolve(project, &NamedNode::new(scope)?, &writer)
                .await;
            drop(writer);
            task.await??;
            println!("{}", result?.id.as_str());
        },
        PublicationCommand::Query { store, query } => {
            let query =
                String::from_utf8(bounded_file(query, 64 * 1024).await?)?;
            let dataset = store.open().await?;
            let mut rows = dataset.select(query).await?;
            while let Some(row) = rows.next().await? {
                for (variable, value) in row.iter() {
                    print!("{variable}={value}\t");
                }
                println!();
            }
        },
        PublicationCommand::Export { store, output } => {
            store.open().await?.export(output.clone()).await?
        },
        PublicationCommand::Import { store, input } => {
            store.open().await?.import(input.clone()).await?
        },
    }
    Ok(())
}

async fn bounded_file(path: &PathBuf, limit: u64) -> Result<Vec<u8>> {
    let mut file = tokio::fs::File::open(path).await?.take(limit + 1);
    // bounded: caller selects an explicit per-document byte limit.
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).await?;
    ensure!(
        bytes.len() as u64 <= limit,
        "{} exceeds operation byte limit",
        path.display()
    );
    Ok(bytes)
}
