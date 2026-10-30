//! Discover a Markdown documentation tree, collect independent files in
//! parallel, and assemble a website plan. Rendering only reads that pinned KG.
use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            MarkdownFile,
            RdfFile,
            TextAssetFile,
            collect_all,
        },
        dataset::{Dataset, identity},
        document::{Documentor, MermaidDocumentor},
        plan::{ResolvedPlan, TargetPlanner},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// A complete collected dataset and immutable website plan.
pub struct CollectedWebsite {
    /// Sources, metadata, and planning contributions.
    pub dataset: Dataset,
    /// Renderer input that remains usable after dataset export/import.
    pub plan:    ResolvedPlan,
}

/// Build a website plan from a bounded documentation tree and RDF definitions.
/// Hidden and underscore-prefixed directories are excluded from discovery.
/// Supported sources are Markdown pages and `.ttl`/`.txt` downloads plus CNAME.
pub async fn collect(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedWebsite> {
    collect_with_asset_factory(
        source,
        definitions,
        project,
        target,
        concurrency,
        &super::collect::default_http_image_factory,
    )
    .await
}

/// Build a site with an explicit factory for retained image sources. Source
/// requests are deduplicated across the complete Markdown batch before any
/// specialized image collector runs.
pub async fn collect_with_asset_factory(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
    asset_factory: &super::collect::AssetCollectorFactory<'_>,
) -> Result<CollectedWebsite> {
    let source = tokio::fs::canonicalize(source).await?;
    let definitions = tokio::fs::canonicalize(definitions).await?;
    let paths = discover(&source).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
    let result = async {
        let mut work: Vec<Arc<dyn Collector>> = vec![Arc::new(RdfFile::new(
            project,
            "urn:grapharch:website-definitions",
            definitions.clone(),
        )?)];
        for path in paths {
            if path == definitions {
                continue;
            }
            let relative = super::collect::page::source_path(&source, &path)?;
            let collector =
                identity("website-collector", &[project, &relative]);
            if path.extension().is_some_and(|extension| extension == "md") {
                work.push(Arc::new(MarkdownFile::in_site(
                    project,
                    collector.as_str(),
                    source.clone(),
                    path,
                )?));
            } else {
                work.push(Arc::new(TextAssetFile::new(
                    project,
                    collector.as_str(),
                    source.clone(),
                    path,
                )?));
            }
        }
        collect_all(work, &writer, concurrency).await?;
        let images = super::collect::http_image_collectors_with_factory(
            &dataset,
            project,
            asset_factory,
        )
        .await?;
        collect_all(images, &writer, concurrency).await?;

        super::ontology_specification::collect(
            &source, &dataset, &writer, project,
        )
        .await?;
        let source_view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for(project).await?,
        );
        if let Some(value) = source_view
            .optional(&NamedNode::new(project)?, "showcasesSource")
            .await?
        {
            let oxrdf::Term::Literal(path) = value else {
                anyhow::bail!(
                    "showcasesSource must be a directory path literal"
                )
            };
            let root =
                tokio::fs::canonicalize(source.join(path.value())).await?;
            super::gallery::collect(&root, &dataset, &writer, project).await?;
        }
        bind(&dataset, &writer, project, target).await?;
        let revision =
            Assembler::new(dataset.clone()).assemble(project).await?;
        TargetPlanner::new(dataset.clone(), revision)
            .resolve(project, &NamedNode::new(target)?, &writer)
            .await
    }
    .await;
    drop(writer);
    worker.await??;
    Ok(CollectedWebsite { dataset, plan: result? })
}

async fn discover(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            visited += 1;
            ensure!(
                visited <= 4096,
                "website discovery exceeds 4096-entry budget"
            );
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                anyhow::anyhow!("website filenames must be UTF-8")
            })?;
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let kind = entry.file_type().await?;
            ensure!(
                !kind.is_symlink(),
                "website discovery rejects symlinks: {}",
                entry.path().display()
            );
            if kind.is_dir() {
                pending.push(entry.path());
            } else if name == "CNAME" ||
                entry.path().extension().is_some_and(|ext| {
                    matches!(ext.to_str(), Some("md" | "ttl" | "txt"))
                })
            {
                files.push(entry.path());
                ensure!(
                    files.len() <= 256,
                    "website exceeds 256-source budget"
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) async fn bind(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    target: &str,
) -> Result<()> {
    MermaidDocumentor::new(
        dataset.clone(),
        dataset.snapshot_collected_for(project).await?,
        project,
    )?
    .document(writer)
    .await?;
    let snapshot = dataset.snapshot_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let target = NamedNode::new(target)?;
    ensure!(
        view.text(&target, "renderer").await? == "website",
        "build-site requires a website target"
    );
    let scope = view.node(&target, "scope").await?;
    let profile = view.node(&target, "profile").await?;
    let pattern = view.pattern(
        &format!("?fragment a ?kind ; {} ?path", term("sourcePath")),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?fragment ?path WHERE {{ VALUES ?kind {{ {} {} \
             {} }} {pattern} }} ORDER BY ?path",
            term("Page"),
            term("TextAsset"),
            term("BinaryAsset")
        ))
        .await?;
    let owner = identity("website-bindings", &[project, target.as_str()]);
    let mut stream = writer
        .begin(
            project,
            owner.as_str(),
            target.as_str(),
            &format!("website-bindings-v3:{snapshot}"),
        )
        .await?;
    let mut order = 0u32;
    while let Some(row) = rows.next().await? {
        let oxrdf::Term::NamedNode(fragment) = &row["fragment"] else {
            anyhow::bail!("website fragment must be an IRI")
        };
        if view.has_type(fragment, "BinaryAsset").await? {
            stream
                .write(Quad::new(
                    scope.clone(),
                    term("include"),
                    fragment.clone(),
                    GraphName::DefaultGraph,
                ))
                .await?;
            continue;
        }
        order += 1;
        ensure!(
            order <= 256,
            "website exceeds 256-fragment budget"
        );
        let component = identity("website-component", &[
            target.as_str(),
            fragment.as_str(),
        ]);
        let placement = identity("website-placement", &[
            target.as_str(),
            fragment.as_str(),
        ]);
        let title = super::render::page_title(&view, fragment).await?;
        let bindings: [(NamedNode, NamedNode, oxrdf::Term); 6] = [
            (
                profile.clone(),
                term("placement"),
                placement.clone().into(),
            ),
            (
                placement.clone(),
                NamedNode::new_unchecked(super::RDF_TYPE),
                term("Placement").into(),
            ),
            (
                placement.clone(),
                term("component"),
                component.clone().into(),
            ),
            (
                placement,
                term("order"),
                Literal::from(order).into(),
            ),
            (
                component.clone(),
                NamedNode::new_unchecked(super::RDF_TYPE),
                term("Component").into(),
            ),
            (
                component.clone(),
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                Literal::new_simple_literal(title).into(),
            ),
        ];
        for (subject, predicate, object) in bindings {
            stream
                .write(Quad::new(
                    subject,
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        if view.has_type(fragment, "Page").await? {
            let mut topics = view.objects(fragment, "topic").await?;
            while let Some(row) = topics.next().await? {
                let oxrdf::Term::NamedNode(topic) = &row["value"] else {
                    anyhow::bail!("topic must be an IRI")
                };
                let part = view.node(topic, "fragment").await?;
                stream
                    .write(Quad::new(
                        scope.clone(),
                        term("include"),
                        topic.clone(),
                        GraphName::DefaultGraph,
                    ))
                    .await?;
                stream
                    .write(Quad::new(
                        component.clone(),
                        term("fragment"),
                        part,
                        GraphName::DefaultGraph,
                    ))
                    .await?;
            }
        } else {
            stream
                .write(Quad::new(
                    scope.clone(),
                    term("include"),
                    fragment.clone(),
                    GraphName::DefaultGraph,
                ))
                .await?;
            stream
                .write(Quad::new(
                    component,
                    term("fragment"),
                    fragment.clone(),
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
    }
    stream.complete().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn website_targets_retain_independent_binding_contributions()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().to_owned();
        tokio::fs::write(
            root.join("index.md"),
            "---\ntitle: Home\ngrapharch:\n  subject: \
             urn:website:home\n---\n# Home\nShared content.\n",
        )
        .await?;
        let definitions = root.join("project.ttl");
        tokio::fs::write(
            &definitions,
            "@prefix ga: <https://ekgf.org/ontology/grapharch#> . \
             <urn:project> a ga:Project ; ga:target <urn:one>, <urn:two> . \
             <urn:one> a ga:Target ; ga:renderer \"website\" ; ga:scope \
             <urn:scope:one> ; ga:profile <urn:profile:one> ; ga:destination \
             \"one\" . <urn:two> a ga:Target ; ga:renderer \"website\" ; \
             ga:scope <urn:scope:two> ; ga:profile <urn:profile:two> ; \
             ga:destination \"two\" . <urn:scope:one> a ga:Scope . \
             <urn:scope:two> a ga:Scope . <urn:profile:one> a ga:Profile . \
             <urn:profile:two> a ga:Profile .",
        )
        .await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
        RdfFile::new("urn:project", "urn:definitions", definitions)?
            .collect(&writer)
            .await?;
        MarkdownFile::in_site(
            "urn:project",
            "urn:page",
            root.clone(),
            root.join("index.md"),
        )?
        .collect(&writer)
        .await?;
        bind(&dataset, &writer, "urn:project", "urn:one").await?;
        bind(&dataset, &writer, "urn:project", "urn:two").await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:project").await?,
        );
        for profile in ["urn:profile:one", "urn:profile:two"] {
            let mut placements =
                view.objects(&NamedNode::new(profile)?, "placement").await?;
            assert!(
                placements.next().await?.is_some(),
                "bindings for {profile} were replaced by another website \
                 target"
            );
        }
        drop(writer);
        worker.await??;
        Ok(())
    }
}
