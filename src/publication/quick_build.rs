//! Zero-configuration entry point using the shared collection and publication
//! stages.
mod aliases;
mod discover;
mod document;
pub(crate) mod pinned;

use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            LocalImageFile,
            MarkdownFile,
            RdfFile,
            collect_all,
            source_checkout::SourceCheckout,
        },
        dataset::{Dataset, identity},
        plan::{ResolvedPlan, TargetPlanner},
        reference::LinkPolicy,
        render::{PublishOutcome, Publisher},
        repository::{ForgeAccess, RepositoryDiscoverer, RepositoryRecord},
        source_link::{CommitId, RepositoryPath},
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    clap::Args,
    oxrdf::{Literal, NamedNode, Term},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// Build ordinary source files without requiring publication definitions.
#[derive(Args)]
pub struct BuildRequest {
    /// An RDF/Markdown file or directory (defaults to the current directory).
    #[arg(default_value = ".")]
    pub source:  PathBuf,
    /// Parent of the generated site and retained documentation graph.
    #[arg(short, long, default_value = "output/grapharch")]
    pub output:  PathBuf,
    #[command(flatten)]
    pub options: BuildOptions,
}

/// How a zero-configuration build treats its repository.
#[derive(Args, Clone, Debug, Default)]
pub struct BuildOptions {
    /// Most sources discovery selects before failing (default 16384).
    #[arg(long, value_name = "COUNT")]
    pub max_sources: Option<u64>,
    /// Never ask the forge API; use Git and package manifests only.
    #[arg(long)]
    pub offline:     bool,
    /// Fail on the first broken link instead of rendering it as text.
    #[arg(long)]
    pub strict:      bool,
}

impl BuildOptions {
    /// Git and manifests only, lenient links: reproducible and network-free.
    pub fn offline() -> Self { Self { offline: true, ..Self::default() } }
}

impl BuildRequest {
    /// Collect, plan, and publish; invalid explicit configuration is never
    /// ignored.
    pub async fn build(&self) -> Result<()> {
        let (dataset, plan) = self.collect().await?;
        let PublishOutcome::Published(publication) =
            Publisher::new(dataset.clone())
                .with_edition(crate::publication::pdf_name::Edition::Default)
                .render(&plan, &self.output)
                .await?
        else {
            anyhow::bail!("publication was superseded")
        };
        let graph = self.output.join("documentation.nq");
        dataset.export(graph.clone()).await?;
        // Reference decisions join the documentation graph in their own named
        // graph, so links are queryable beside the content they connect.
        let references =
            publication.directory.join(super::reference::RECORD_FILE);
        if tokio::fs::try_exists(&references).await? {
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(&graph)
                .await?;
            tokio::io::AsyncWriteExt::write_all(
                &mut file,
                &tokio::fs::read(&references).await?,
            )
            .await?;
            tokio::io::AsyncWriteExt::flush(&mut file).await?;
        }
        println!(
            "Open {}",
            tokio::fs::canonicalize(publication.directory.join("index.html"))
                .await?
                .display()
        );
        Ok(())
    }

    /// Retain the complete plan so it can be rendered after source removal.
    pub async fn collect(&self) -> Result<(Dataset, ResolvedPlan)> {
        let source = tokio::fs::canonicalize(&self.source).await?;
        let budget = self
            .options
            .max_sources
            .map(super::source_walk::SourceBudget::try_new)
            .transpose()?
            .unwrap_or_default();
        let inventory =
            discover::Inventory::read(&source, &self.output, budget).await?;
        if let Some(definitions) = &inventory.definitions {
            return self.configured(&inventory.root, definitions).await;
        }
        if let Some(path) = &inventory.mkdocs {
            let configuration =
                super::collect::mkdocs::MkDocs::new(&inventory.root, path)
                    .await?;
            let project = configuration.project.clone();
            let target = configuration.target.clone();
            let built = super::website::collect_with_configuration(
                &inventory.root,
                path,
                project.as_str(),
                target.as_str(),
                4,
                &super::collect::default_http_image_factory,
                super::website::SourceConfiguration {
                    collector:        Arc::new(configuration),
                    mkdocs_metadata:  true,
                    repository:       None,
                    extra_collectors: Vec::new(),
                },
            )
            .await?;
            return Ok((built.dataset, built.plan));
        }
        let origin = url::Url::from_file_path(&source)
            .map_err(|_| anyhow::anyhow!("invalid source path"))?;
        let project = identity("quick-project", &[origin.as_str()]);
        let facts = RepositoryDiscoverer::new(
            if self.options.offline {
                ForgeAccess::Offline
            } else {
                ForgeAccess::Online
            },
        )
        .discover(&inventory.root)
        .await;
        let mut extra: Vec<Arc<dyn Collector>> = Vec::new(); // bounded: provenance and repository facts.
        if let Some(repository) = super::collect::git::enclosing_repository(
            &inventory.root,
            project.as_str(),
        )
        .await?
        {
            extra.push(repository.provenance);
        }
        if let Some(git) = &facts.git {
            extra.push(Arc::new(SourceCheckout::new(
                project.clone(),
                CommitId::try_new(git.head.as_str())?,
                RepositoryPath::within(&git.root, &inventory.root)?,
            )));
        }
        let repository = Repository {
            name:    facts.name(),
            package: facts.has_package(),
            web:     facts.forge.as_ref().map(|forge| forge.web_url()),
            policy:  if self.options.strict {
                LinkPolicy::Strict
            } else {
                LinkPolicy::Lenient
            },
        };
        extra.push(Arc::new(RepositoryRecord::new(
            project.clone(),
            origin.to_string(),
            facts,
        )));
        plain(&inventory, project, extra, &repository).await
    }

    async fn configured(
        &self,
        root: &Path,
        definitions: &Path,
    ) -> Result<(Dataset, ResolvedPlan)> {
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
        let result = RdfFile::new(
            "urn:grapharch:configuration-inspection",
            "urn:grapharch:configuration-inspection",
            definitions.to_owned(),
        )?
        .collect(&writer)
        .await;
        drop(writer);
        worker.await??;
        result?;
        let view =
            RevisionView::new(dataset.clone(), dataset.snapshot().await?);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?project ?target ?renderer WHERE {{ {} }} \
                 LIMIT 3",
                view.pattern(
                    &format!(
                        "?project a {} ; {} ?target . ?target {} ?renderer",
                        super::term("Project"),
                        super::term("target"),
                        super::term("renderer")
                    ),
                    0
                )
            ))
            .await?;
        let row = rows.next().await?.ok_or_else(|| {
            anyhow::anyhow!(
                "{} must declare a ga:Project and target; use explicit \
                 publication commands for a multi-target configuration",
                definitions.display()
            )
        })?;
        ensure!(
            rows.next().await?.is_none(),
            "multiple configured targets: choose one with publication \
             build-site/build-family --project ... --target ..."
        );
        let (
            Term::NamedNode(project),
            Term::NamedNode(target),
            Term::NamedNode(renderer),
        ) = (&row["project"], &row["target"], &row["renderer"])
        else {
            anyhow::bail!("invalid configured publication identities")
        };
        if super::Renderer::from_node(renderer)? == super::Renderer::Website {
            let built = super::website::collect(
                root,
                definitions,
                project.as_str(),
                target.as_str(),
                4,
            )
            .await?;
            Ok((built.dataset, built.plan))
        } else {
            let source = if super::collect::git::enclosing_repository(
                root,
                project.as_str(),
            )
            .await?
            .is_some()
            {
                super::ontology_family::FamilySource::LocalRepository(root)
            } else {
                super::ontology_family::FamilySource::Directory(root)
            };
            let built = super::ontology_family::collect_from(
                Dataset::memory()?,
                source,
                definitions,
                project.as_str(),
                target.as_str(),
                4,
            )
            .await?;
            Ok((built.dataset, built.plan))
        }
    }
}

fn quad(
    subject: &NamedNode,
    field: &str,
    object: impl Into<Term>,
) -> oxrdf::Quad {
    oxrdf::Quad::new(
        subject.clone(),
        if field == "type" {
            NamedNode::new_unchecked(super::RDF_TYPE)
        } else {
            super::term(field)
        },
        object,
        oxrdf::GraphName::DefaultGraph,
    )
}
fn title_quad(subject: &NamedNode, title: &str) -> oxrdf::Quad {
    oxrdf::Quad::new(
        subject.clone(),
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        Literal::new_simple_literal(title),
        oxrdf::GraphName::DefaultGraph,
    )
}

/// What the zero-configuration defaults take from the repository facts.
struct Repository {
    name:    Option<String>,
    package: bool,
    web:     Option<url::Url>,
    policy:  LinkPolicy,
}

/// Collect an unconfigured tree of Markdown, RDF and images as one website.
async fn plain(
    inventory: &discover::Inventory,
    project: NamedNode,
    extra: Vec<Arc<dyn Collector>>,
    repository: &Repository,
) -> Result<(Dataset, ResolvedPlan)> {
    let target = identity("quick-target", &[project.as_str()]);
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
    let result = async {
        let mut inputs: Vec<Arc<dyn Collector>> = Vec::new(); // bounded: the discovery source budget.
        for path in &inventory.files {
            let relative =
                super::collect::page::source_path(&inventory.root, path)?;
            let collector =
                identity("quick-source", &[project.as_str(), &relative]);
            if path.extension().is_some_and(|ext| ext == "md") {
                inputs.push(Arc::new(
                    MarkdownFile::in_site(
                        project.as_str(),
                        collector.as_str(),
                        inventory.root.clone(),
                        path.clone(),
                    )?
                    .with_default_title(),
                ));
            } else if LocalImageFile::supported(path) {
                inputs.push(Arc::new(
                    LocalImageFile::new(
                        project.as_str(),
                        &inventory.root,
                        path,
                    )
                    .await?,
                ));
            } else {
                inputs.push(Arc::new(RdfFile::new(
                    project.as_str(),
                    collector.as_str(),
                    path.clone(),
                )?));
            }
        }
        inputs.extend(extra);
        collect_all(inputs, &writer, 4).await?;
        let snapshot = dataset.snapshot_collected_for(project.as_str()).await?;
        let view = RevisionView::new(dataset.clone(), snapshot.clone());
        let mut projects = dataset
            .select(format!(
                "SELECT ?p WHERE {{ {} }} LIMIT 1",
                view.pattern(&format!("?p a {}", super::term("Project")), 0)
            ))
            .await?;
        ensure!(
            projects.next().await?.is_none(),
            "publication configuration was found outside \
             grapharch.ttl/project.ttl; use publication build-family or \
             build-site with explicit identifiers"
        );
        let title = document::title(
            &dataset,
            &view,
            inventory,
            repository.name.as_deref(),
            repository.package,
        )
        .await?;
        document::defaults(
            &writer,
            &project,
            &target,
            &title,
            repository.web.as_ref(),
            repository.policy,
        )
        .await?;
        document::rdf_pages(&dataset, &writer, &project, inventory).await?;
        document::home(&dataset, &writer, &project, &title).await?;
        let images =
            super::collect::http_image_collectors(&dataset, project.as_str())
                .await?;
        collect_all(images, &writer, 4).await?;
        super::website::bind(
            &dataset,
            &writer,
            project.as_str(),
            target.as_str(),
        )
        .await?;
        let revision = Assembler::new(dataset.clone())
            .assemble(project.as_str())
            .await?;
        TargetPlanner::new(dataset.clone(), revision)
            .resolve(project.as_str(), &target, &writer)
            .await
    }
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}
