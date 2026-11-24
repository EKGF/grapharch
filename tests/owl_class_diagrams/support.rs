//! Shared fixture for one documented class diagram.
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

/// The documentation body of `https://example.org/{focus}`, compacting IRIs
/// with the given namespace prefixes.
pub(super) async fn diagram_with_prefixes(
    source: &str,
    prefixes: &[(&str, &str)],
    focus: &str,
) -> Result<String> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("ontology.ttl");
    tokio::fs::write(
        &path,
        format!(
            r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
{source}
"#
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:diagram:project", "urn:diagram:source", path)?
        .collect(&writer)
        .await?;
    let snapshot = dataset.snapshot_for("urn:diagram:project").await?;
    let documentor = AnnotatedTermDocumentor::new(
        dataset.clone(),
        snapshot,
        "urn:diagram:project",
        NamedNode::new(format!("https://example.org/{focus}"))?,
        "en",
        Arc::new(
            prefixes
                .iter()
                .map(|(namespace, prefix)| {
                    ((*namespace).to_owned(), (*prefix).to_owned())
                })
                .collect::<BTreeMap<_, _>>(),
        ),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    documentor.document(&writer).await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
    );
    let body = view.text(&documentor.fragment_id(), "body").await?;
    drop(writer);
    worker.await??;
    Ok(body)
}

/// One retained projection: its PlantUML source, accessible description and
/// semantic input.
#[derive(serde::Deserialize)]
pub(super) struct Drawing {
    pub source:      String,
    pub description: String,
    pub diagram:     serde_json::Value,
}
impl Drawing {
    pub fn parse(body: &str) -> Result<Self> {
        Ok(serde_json::from_str(
            body.split("```grapharch-class-diagram\n")
                .nth(1)
                .expect("a class diagram")
                .split("\n```")
                .next()
                .expect("a closed class diagram block"),
        )?)
    }

    /// Lines of the PlantUML source containing every given fragment.
    pub fn lines(&self, fragments: &[&str]) -> Vec<&str> {
        self.source
            .lines()
            .filter(|line| fragments.iter().all(|f| line.contains(f)))
            .collect()
    }

    /// The source states no raw OWL, RDF or RDFS construct.
    pub fn assert_uml_only(&self) {
        for raw in [
            "owl:",
            "rdfs:",
            "rdf:",
            "Anonymous OWL",
            "OWL expression",
            "OWL restriction",
            "Literal value",
            "<<Resource>>",
        ] {
            assert!(
                !self.source.contains(raw),
                "{raw} in {}",
                self.source
            );
        }
    }
}
