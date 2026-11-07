//! Explicit source dependencies, independent of document and renderer kinds.
use {
    super::{Collector, RdfFile, RdfUrl, SourceIdentity, collect_all, git},
    crate::publication::{
        dataset::Dataset,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Context, Result, bail, ensure},
    async_trait::async_trait,
    oxrdf::{NamedNode, Term},
    std::{
        path::{Component, Path, PathBuf},
        sync::Arc,
    },
};

pub(crate) async fn collect(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    concurrency: usize,
) -> Result<()> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project).await?,
    );
    let mut definitions =
        view.objects(&NamedNode::new(project)?, "collector").await?;
    // bounded: 32 explicit source definitions expand to at most 512 collector
    // instances.
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    let mut count = 0;
    while let Some(row) = definitions.next().await? {
        count += 1;
        ensure!(
            count <= 32,
            "configured sources exceed 32-source budget"
        );
        let Term::NamedNode(processor) = &row["value"] else {
            bail!("configured collector must be an IRI")
        };
        ensure!(
            view.has_type(processor, "Collector").await?,
            "configured source must be typed Collector"
        );
        let kind = view.text(processor, "kind").await?;
        let source = view.text(processor, "source").await?;
        if kind == "authored-use-cases" {
            work.extend(
                super::authored_use_cases::configured(
                    project, processor, &source, root, &view,
                )
                .await?,
            );
            ensure!(
                work.len() <= 512,
                "configured sources exceed 512 collector instances"
            );
            continue;
        }
        let collector: Arc<dyn Collector> = match kind.as_str() {
            "abbreviation-file" => Arc::new(super::abbreviations::AbbreviationFile::configured(project, processor, &source, root).await?),
            "repository-publications" => Arc::new(super::repository_publications::RepositoryPublications::configured(project, processor, &source, &view).await?),
            "documentation-navigation" => {
                let relative = Path::new(&source);
                ensure!(relative.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir)), "navigation source must be relative without traversal");
                let path = tokio::fs::canonicalize(root.join(relative)).await?;
                ensure!(path.starts_with(root) && tokio::fs::metadata(&path).await?.is_dir(), "navigation source must be a directory within project root");
                let project_id = NamedNode::new(project)?;
                let content_root = crate::publication::website::discovery::content_root(root, &view, &project_id).await?;
                let exclusions = crate::publication::website::discovery::exclusions(&content_root, &view, &project_id).await?;
                Arc::new(super::pages_navigation::PagesNavigation { exclusions, project: project.into(), processor: processor.clone(), profile: view.node(processor, "navigationProfile").await?, root: path })
            },
            "plantuml-file" => Arc::new(super::plantuml::PlantUmlFile::configured(project, processor, &source, root, &view).await?),
            "rdf-file" => {
                let relative = Path::new(&source);
                ensure!(
                    !source.is_empty() &&
                        relative.components().all(|part| {
                            matches!(part, Component::Normal(_))
                        }),
                    "configured RDF source must be relative without traversal"
                );
                let path = tokio::fs::canonicalize(root.join(relative))
                    .await
                    .with_context(|| {
                    format!("configured RDF source {source}")
                })?;
                ensure!(
                    path.starts_with(root),
                    "configured RDF source escapes documentation root"
                );
                Arc::new(RdfFile::new(project, processor.as_str(), path)?)
            },
            "rdf-url" => {
                Arc::new(RdfUrl::new(
                    project,
                    processor.as_str(),
                    &source,
                    &view.text(processor, "format").await?,
                )?)
            },
            "git-rdf-file" => {
                let path = view.text(processor, "sourcePath").await?;
                let reference = view.text(processor, "gitRef").await?;
                ensure!(
                    !reference.trim().is_empty(),
                    "configured Git source requires gitRef"
                );
                ensure!(
                    !path.is_empty() &&
                        Path::new(&path).components().all(|part| {
                            matches!(part, Component::Normal(_))
                        }),
                    "configured Git sourcePath must be relative without \
                     traversal"
                );
                Arc::new(GitRdfDependency {
                    project: project.into(),
                    processor: processor.to_owned(),
                    source,
                    path: path.into(),
                    reference,
                })
            },
            _ => bail!("unsupported configured source kind {kind}"),
        };
        work.push(collector);
        ensure!(
            work.len() <= 512,
            "configured sources exceed 512 collector instances"
        );
    }
    collect_all(work, writer, concurrency).await
}

struct GitRdfDependency {
    project:   String,
    processor: NamedNode,
    source:    String,
    path:      PathBuf,
    reference: String,
}

#[async_trait]
impl Collector for GitRdfDependency {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: self.processor.as_str(),
            source:    &self.source,
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let repository = git::remote_repository(
            &self.source,
            &self.reference,
            Path::new("output/cache/git"),
            &self.project,
        )
        .await?;
        repository.provenance.produce(output).await?;
        repository
            .blob(&self.path)
            .await?
            .rdf_collector(self.processor.as_str())?
            .produce(output)
            .await
    }
}
