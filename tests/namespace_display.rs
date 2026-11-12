#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        render::{PublishOutcome, Publisher},
        revision_build::TargetSelection,
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

#[tokio::test]
async fn compact_identifiers_use_the_most_specific_namespace() -> Result<()> {
    let source = tempfile::tempdir()?;
    let file = source.path().join("term.ttl");
    tokio::fs::write(&file, r#"<https://example.org/vocab/shapes/Thing> <http://www.w3.org/2000/01/rdf-schema#label> "Thing" ; <http://purl.org/dc/terms/description> "A shape." ."#).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:names:project", "urn:names:collector", file)?
        .collect(&writer)
        .await?;
    let job = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:names:project").await?,
        "urn:names:project",
        NamedNode::new("https://example.org/vocab/shapes/Thing")?,
        "en",
        Arc::new(BTreeMap::from([
            (
                "https://example.org/vocab/".into(),
                "vocab".into(),
            ),
            (
                "https://example.org/vocab/shapes/".into(),
                "shapes".into(),
            ),
        ])),
        Arc::new(HashSet::new()),
    )?;
    job.document(&writer).await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:names:project").await?,
    );
    let body = view.text(&job.fragment_id(), "body").await?;
    assert!(body.starts_with("## shapes:Thing\n"), "{body}");
    assert!(!body.contains("vocab:shapes/Thing"));
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn dprod_reference_honours_source_declared_nested_shape_namespaces()
-> Result<()> {
    let mut fixture = revision_fixture::RevisionFixture::new(
        std::path::Path::new("examples/dprod"),
    )?;
    fixture.edit_content("shapes.ttl", |mut data| {
        data.push_str(r#"
<https://www.omg.org/spec/DPROD/dprod/contracts/shapes/>
 a owl:Ontology ;
 sh:declare [ sh:prefix "contracts-shapes" ; sh:namespace "https://www.omg.org/spec/DPROD/dprod/contracts/shapes/" ] .
<https://www.omg.org/spec/DPROD/dprod/contracts/shapes/DataContractShape>
 a sh:NodeShape ; rdfs:label "Data contract shape"@en ;
 dct:description "Constraints on a contract."@en .
"#);
        Ok(data)
    })?;
    let (dataset, plans) = fixture
        .collect(TargetSelection::targets(vec![NamedNode::new(
            "urn:dprod-showcase:specification",
        )?])?)
        .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    fixture.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("contracts-shapes:DataContractShape"));
    assert!(!html.contains("dprod:contracts/shapes/DataContractShape"));
    Ok(())
}
