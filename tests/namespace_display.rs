use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        render::{PublishOutcome, Publisher},
        showcases,
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
    let source = tempfile::tempdir()?;
    for path in [
        "project.ttl",
        "abstract.md",
        "status.md",
        "overview.md",
        "sources/ontology.ttl",
        "sources/shapes.ttl",
        "sources/dataset-schema.md",
        "website/index.md",
        "website/stories.md",
        "website/guides/getting-started.md",
    ] {
        let destination = source.path().join(path);
        tokio::fs::create_dir_all(destination.parent().unwrap()).await?;
        tokio::fs::copy(
            std::path::Path::new("examples/dprod").join(path),
            destination,
        )
        .await?;
    }
    let path = source.path().join("sources/shapes.ttl");
    let mut data = tokio::fs::read_to_string(&path).await?;
    data.push_str(r#"
<https://www.omg.org/spec/DPROD/dprod/contracts/shapes/>
 a owl:Ontology ;
 sh:declare [ sh:prefix "contracts-shapes" ; sh:namespace "https://www.omg.org/spec/DPROD/dprod/contracts/shapes/" ] .
<https://www.omg.org/spec/DPROD/dprod/contracts/shapes/DataContractShape>
 a sh:NodeShape ; rdfs:label "Data contract shape"@en ;
 dct:description "Constraints on a contract."@en .
"#);
    tokio::fs::write(path, data).await?;
    let (dataset, plans) = showcases::dprod(source.path()).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
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
