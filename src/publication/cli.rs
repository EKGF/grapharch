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
    clap::{Subcommand, ValueEnum},
    oxigraph::{io::RdfFormat, store::Store},
    oxrdf::{NamedNode, Term},
    std::path::PathBuf,
    tokio::io::AsyncReadExt,
};

/// Independently executable publication operations using a persistent shared
/// dataset.
#[derive(Subcommand)]
pub enum PublicationCommand {
    /// Render a pinned target plan without rereading its original sources.
    Render {
        #[arg(long)]
        store:       PathBuf,
        #[arg(long)]
        plan:        String,
        #[arg(long)]
        snapshot:    String,
        #[arg(long)]
        output_root: PathBuf,
    },
    /// Assemble and atomically promote a validated project revision.
    Assemble {
        #[arg(long)]
        store:   PathBuf,
        #[arg(long)]
        project: String,
    },
    /// Resolve one target or every registered target from a validated revision.
    Plan {
        #[arg(long)]
        store:    PathBuf,
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
    /// Collect one source through a bounded writer into an embedded dataset.
    Collect {
        #[arg(long)]
        store:      PathBuf,
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
        #[arg(long)]
        store: PathBuf,
    },
    /// Resolve a scope against an explicit pinned snapshot.
    ResolveScope {
        #[arg(long)]
        store:    PathBuf,
        #[arg(long)]
        project:  String,
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        scope:    String,
    },
    /// Stream an explicit inspection SELECT query from a UTF-8 query file.
    Query {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        query: PathBuf,
    },
    /// Export a complete dataset, including contributions and pinned snapshots.
    Export {
        #[arg(long)]
        store:  PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Import into an empty dataset; existing data is never merged implicitly.
    Import {
        #[arg(long)]
        store: PathBuf,
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
    W3cSpecification,
}

/// Dispatch one operation. No source access is required for
/// snapshot/query/resolve/export.
pub async fn run(
    command: &PublicationCommand,
    mode: FailureMode,
) -> Result<()> {
    match command {
        PublicationCommand::Render { store, plan, snapshot, output_root } => {
            let dataset = Dataset::open(store.clone()).await?;
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
            let dataset = Dataset::open(store.clone()).await?;
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
            let dataset = Dataset::open(store.clone()).await?;
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
            let dataset = Dataset::open(store.clone()).await?;
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
            let dataset = Dataset::open(store.clone()).await?;
            println!("{}", dataset.snapshot().await?.as_str());
        },
        PublicationCommand::ResolveScope {
            store,
            project,
            snapshot,
            scope,
        } => {
            let dataset = Dataset::open(store.clone()).await?;
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
            let dataset = Dataset::open(store.clone()).await?;
            let mut rows = dataset.select(query).await?;
            while let Some(row) = rows.next().await? {
                for (variable, value) in row.iter() {
                    print!("{variable}={value}\t");
                }
                println!();
            }
        },
        PublicationCommand::Export { store, output } => {
            Dataset::open(store.clone())
                .await?
                .export(output.clone())
                .await?
        },
        PublicationCommand::Import { store, input } => {
            Dataset::open(store.clone())
                .await?
                .import(input.clone())
                .await?
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
