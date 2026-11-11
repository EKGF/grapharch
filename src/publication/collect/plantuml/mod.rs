//! One explicit PlantUML source, its local include closure, and its SVG
//! identity.
mod source;
use {
    super::{Collector, SourceIdentity},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::{
        collections::BTreeMap,
        path::{Component, Path, PathBuf},
        sync::Arc,
    },
};

/// A specialized source collector; rendering is a separate pinned-KG document
/// stage.
pub struct PlantUmlFile {
    project:   String,
    processor: String,
    root:      PathBuf,
    path:      PathBuf,
    source:    String,
    output:    String,
    asset:     NamedNode,
}
fn output_path(
    root: &Path,
    content: &Path,
    destination: &str,
) -> Result<String> {
    let path = Path::new(destination);
    ensure!(
        !destination.is_empty() &&
            path.components().all(|p| matches!(p, Component::Normal(_))) &&
            path.extension().is_some_and(|ext| ext == "svg"),
        "PlantUML destination must be a safe project-relative .svg path"
    );
    let absolute = root.join(path);
    super::page::source_path(content, &absolute)
}
impl PlantUmlFile {
    /// Resolve one explicitly configured local source. No generated SVG must
    /// exist.
    pub async fn configured(
        project: &str,
        processor: &NamedNode,
        source: &str,
        root: &Path,
        view: &RevisionView,
    ) -> Result<Self> {
        let root = tokio::fs::canonicalize(root).await?;
        let content = match view
            .optional(&NamedNode::new(project)?, "contentSource")
            .await?
        {
            Some(Term::Literal(value)) => {
                tokio::fs::canonicalize(root.join(value.value())).await?
            },
            None => root.clone(),
            _ => anyhow::bail!("contentSource must be a path literal"),
        };
        ensure!(
            content.starts_with(&root),
            "contentSource escapes project root"
        );
        let output = output_path(
            &root,
            &content,
            &view.text(processor, "destination").await?,
        )?;
        let path = Path::new(source);
        ensure!(
            path.components().all(|p| matches!(p, Component::Normal(_))) &&
                path.extension().is_some_and(|ext| ext == "puml"),
            "PlantUML source must be a project-relative .puml file"
        );
        let path = tokio::fs::canonicalize(root.join(path)).await?;
        ensure!(
            path.starts_with(&root),
            "PlantUML source escapes project root"
        );
        Ok(Self {
            project: project.into(),
            processor: processor.as_str().into(),
            root,
            path,
            source: source.into(),
            asset: identity("local-image-asset", &[project, &output]),
            output,
        })
    }
}
/// Predeclare bounded generated image identities before Markdown requests are
/// collected.
pub async fn bindings(
    view: &RevisionView,
    project: &str,
    project_root: &Path,
    content_root: &Path,
) -> Result<Arc<BTreeMap<String, NamedNode>>> {
    let mut rows = view.objects(&NamedNode::new(project)?, "collector").await?;
    let mut result = BTreeMap::new(); // bounded: at most 32 explicitly configured collectors.
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 32,
            "configured sources exceed 32-source budget"
        );
        let Term::NamedNode(processor) = &row["value"] else {
            anyhow::bail!("collector must be an IRI");
        };
        if super::kind::CollectorKind::from_node(
            &view.node(processor, "collectorType").await?,
        )? != super::kind::CollectorKind::PlantUmlFile
        {
            continue;
        }
        let destination = output_path(
            project_root,
            content_root,
            &view.text(processor, "destination").await?,
        )?;
        let asset = identity("local-image-asset", &[project, &destination]);
        ensure!(
            result.insert(destination, asset).is_none(),
            "PlantUML outputs collide"
        );
    }
    Ok(Arc::new(result))
}
#[async_trait]
impl Collector for PlantUmlFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let expanded = source::expand(&self.root, &self.path).await?;
        ensure!(
            expanded
                .code
                .lines()
                .filter(|s| s.trim_start().starts_with("@startuml"))
                .count() ==
                1 &&
                expanded
                    .code
                    .lines()
                    .filter(|s| s.trim() == "@enduml")
                    .count() ==
                    1,
            "PlantUML file must contain exactly one @startuml/@enduml diagram"
        );
        let diagram =
            identity("plantuml-source", &[&self.project, &self.source]);
        let quad = |id: &NamedNode, predicate: &str, value: Term| {
            Quad::new(
                id.clone(),
                if predicate == "type" {
                    NamedNode::new_unchecked(RDF_TYPE)
                } else {
                    term(predicate)
                },
                value,
                GraphName::DefaultGraph,
            )
        };
        for value in [
            quad(&diagram, "type", term("Example").into()),
            quad(
                &diagram,
                "language",
                Literal::new_simple_literal("plantuml").into(),
            ),
            quad(
                &diagram,
                "body",
                Literal::new_simple_literal(&expanded.code).into(),
            ),
            quad(
                &diagram,
                "sourcePath",
                Literal::new_simple_literal(&self.source).into(),
            ),
            quad(
                &diagram,
                "route",
                Literal::new_simple_literal(&self.output).into(),
            ),
            quad(&diagram, "asset", self.asset.clone().into()),
        ] {
            stream.write(value).await?;
        }
        let mut digest = Sha256::new();
        for (path, body) in expanded.sources {
            let path = super::page::source_path(&self.root, &path)?;
            let source = identity("plantuml-input", &[&self.project, &path]);
            digest.update(path.as_bytes());
            digest.update([0]);
            digest.update(body.as_bytes());
            for value in [
                quad(&source, "type", term("Example").into()),
                quad(
                    &source,
                    "language",
                    Literal::new_simple_literal("plantuml-source").into(),
                ),
                quad(
                    &source,
                    "sourcePath",
                    Literal::new_simple_literal(path).into(),
                ),
                quad(
                    &source,
                    "body",
                    Literal::new_simple_literal(body).into(),
                ),
            ] {
                stream.write(value).await?;
            }
            stream
                .write(Quad::new(
                    diagram.clone(),
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/hasPart",
                    ),
                    source,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(format!(
            "plantuml-source-v1:{}",
            hex::encode(digest.finalize())
        ))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::publication::{
            collect::RdfFile,
            dataset::Dataset,
            writer::GraphWriter,
        },
    };
    #[tokio::test]
    async fn configured_sources_retain_original_includes_and_predeclare_absent_outputs()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        tokio::fs::create_dir_all(root.path().join("docs/theme")).await?;
        let original =
            "@startuml\n!include theme/light.puml\nclass A\n@enduml\n";
        tokio::fs::write(root.path().join("docs/diagram.puml"), original)
            .await?;
        tokio::fs::write(
            root.path().join("docs/theme/light.puml"),
            "skinparam backgroundColor transparent\n",
        )
        .await?;
        let definitions = root.path().join("project.ttl");
        tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:uml:project> a ga:Project; ga:contentSource "docs"; ga:collector <urn:uml:collector> .
<urn:uml:collector> a ga:CollectorUse; ga:collectorType ga:PlantUmlFileCollector; ga:source "docs/diagram.puml"; ga:destination "docs/diagrams/out/diagram.svg" ."#).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        RdfFile::new(
            "urn:uml:project",
            "urn:uml:definitions",
            definitions,
        )?
        .collect(&writer)
        .await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:uml:project").await?,
        );
        let map = bindings(
            &view,
            "urn:uml:project",
            root.path(),
            &root.path().join("docs"),
        )
        .await?;
        assert_eq!(map.len(), 1);
        assert!(
            !tokio::fs::try_exists(
                root.path().join("docs/diagrams/out/diagram.svg")
            )
            .await?
        );
        let collector = PlantUmlFile::configured(
            "urn:uml:project",
            &NamedNode::new("urn:uml:collector")?,
            "docs/diagram.puml",
            root.path(),
            &view,
        )
        .await?;
        collector.collect(&writer).await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:uml:project").await?,
        );
        let diagram = identity("plantuml-source", &[
            "urn:uml:project",
            "docs/diagram.puml",
        ]);
        assert!(
            view.text(&diagram, "body")
                .await?
                .contains("skinparam backgroundColor transparent")
        );
        let source = identity("plantuml-input", &[
            "urn:uml:project",
            "docs/diagram.puml",
        ]);
        assert_eq!(view.text(&source, "body").await?, original);
        assert_eq!(
            view.node(&diagram, "asset").await?,
            map["diagrams/out/diagram.svg"]
        );
        drop(writer);
        worker.await??;
        Ok(())
    }
}
