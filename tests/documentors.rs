use {
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{Documentor, OwlClassDocumentor},
        term,
        writer::GraphWriter,
    },
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

#[tokio::test]
async fn owl_class_documentor_generates_source_linked_fragments_without_book_mapping()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ontology.ttl");
    tokio::fs::write(&path, "@prefix owl: <http://www.w3.org/2002/07/owl#> . @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> . <urn:class> a owl:Class ; rdfs:label \"Example class\" ; rdfs:comment \"A documented class.\" .").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:project", "urn:collector:rdf", path)?
        .collect(&writer)
        .await?;
    let input = dataset.snapshot().await?;
    let documentor =
        OwlClassDocumentor::new(dataset.clone(), input, "urn:project")?;
    documentor.document(&writer).await?;
    let output = dataset.snapshot().await?;
    let fragment = documentor.fragment_id(&NamedNode::new("urn:class")?);
    assert!(
        dataset
            .contains(
                &output,
                Quad::new(
                    fragment.clone(),
                    term("subject"),
                    NamedNode::new("urn:class")?,
                    GraphName::DefaultGraph
                )
            )
            .await?
    );
    assert!(
        dataset
            .contains(
                &output,
                Quad::new(
                    fragment,
                    term("body"),
                    Literal::new_simple_literal("A documented class."),
                    GraphName::DefaultGraph
                )
            )
            .await?
    );
    documentor.document(&writer).await?;
    assert_eq!(output, dataset.snapshot().await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn removed_class_does_not_leave_active_generated_fragments()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ontology.ttl");
    tokio::fs::write(
        &path,
        "<urn:class> a <http://www.w3.org/2002/07/owl#Class> .",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let collector =
        RdfFile::new("urn:project", "urn:collector:rdf", path.clone())?;
    collector.collect(&writer).await?;
    let first = OwlClassDocumentor::new(
        dataset.clone(),
        dataset.snapshot().await?,
        "urn:project",
    )?;
    first.document(&writer).await?;
    let fragment = first.fragment_id(&NamedNode::new("urn:class")?);
    let old = dataset.snapshot().await?;
    tokio::fs::write(&path, "# ontology is now empty\n").await?;
    collector.collect(&writer).await?;
    OwlClassDocumentor::new(
        dataset.clone(),
        dataset.snapshot().await?,
        "urn:project",
    )?
    .document(&writer)
    .await?;
    let current = dataset.snapshot().await?;
    let quad = Quad::new(
        fragment,
        term("subject"),
        NamedNode::new("urn:class")?,
        GraphName::DefaultGraph,
    );
    assert!(dataset.contains(&old, quad.clone()).await?);
    assert!(!dataset.contains(&current, quad).await?);
    drop(writer);
    worker.await??;
    Ok(())
}
