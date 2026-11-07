//! Zero-configuration entry point using the shared collection and publication
//! stages.
mod aliases;
mod discover;
mod document;

use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            LocalImageFile,
            MarkdownFile,
            RdfFile,
            collect_all,
        },
        dataset::{Dataset, identity},
        plan::{ResolvedPlan, TargetPlanner},
        render::{PublishOutcome, Publisher},
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
    pub source: PathBuf,
    /// Parent of the generated site and retained documentation graph.
    #[arg(short, long, default_value = "output/grapharch")]
    pub output: PathBuf,
}

impl BuildRequest {
    /// Collect, plan, and publish; invalid explicit configuration is never
    /// ignored.
    pub async fn build(&self) -> Result<()> {
        let (dataset, plan) = self.collect().await?;
        let PublishOutcome::Published(publication) =
            Publisher::new(dataset.clone())
                .render(&plan, &self.output)
                .await?
        else {
            anyhow::bail!("publication was superseded")
        };
        dataset.export(self.output.join("documentation.nq")).await?;
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
        let inventory =
            discover::Inventory::read(&source, &self.output).await?;
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
        let target = identity("quick-target", &[project.as_str()]);
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
        let result = async {
            let mut inputs: Vec<Arc<dyn Collector>> = Vec::new(); // bounded: discovery caps sources at 256.
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
            if let Some(repository) = super::collect::git::enclosing_repository(
                &inventory.root,
                project.as_str(),
            )
            .await?
            {
                inputs.push(repository.provenance);
            }
            collect_all(inputs, &writer, 4).await?;
            let snapshot =
                dataset.snapshot_collected_for(project.as_str()).await?;
            let view = RevisionView::new(dataset.clone(), snapshot.clone());
            let mut projects = dataset
                .select(format!(
                    "SELECT ?p WHERE {{ {} }} LIMIT 1",
                    view.pattern(
                        &format!("?p a {}", super::term("Project")),
                        0
                    )
                ))
                .await?;
            ensure!(
                projects.next().await?.is_none(),
                "publication configuration was found outside \
                 grapharch.ttl/project.ttl; use publication build-family or \
                 build-site with explicit identifiers"
            );
            let title = document::title(&dataset, &view, &inventory).await?;
            document::defaults(&writer, &project, &target, &title).await?;
            document::rdf_pages(&dataset, &writer, &project, &inventory)
                .await?;
            document::home(&dataset, &writer, &project, &title).await?;
            let images = super::collect::http_image_collectors(
                &dataset,
                project.as_str(),
            )
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
            Term::Literal(renderer),
        ) = (&row["project"], &row["target"], &row["renderer"])
        else {
            anyhow::bail!("invalid configured publication identities")
        };
        if renderer.value() == "website" {
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
