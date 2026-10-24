use {
    grapharch::publication::{dataset::Dataset, writer::GraphWriter},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

#[tokio::test]
async fn a_committed_receipt_can_be_pinned_after_a_newer_attempt_replaces_the_head()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let quad = |body: &str| -> anyhow::Result<Quad> {
        Ok(Quad::new(
            NamedNode::new("urn:subject")?,
            NamedNode::new("urn:predicate")?,
            Literal::new_simple_literal(body),
            GraphName::DefaultGraph,
        ))
    };
    let mut first = writer
        .begin("urn:project", "urn:processor", "source", "one")
        .await?;
    first.write(quad("old")?).await?;
    let receipt = first.commit().await?;
    let mut second = writer
        .begin("urn:project", "urn:processor", "source", "two")
        .await?;
    second.write(quad("new")?).await?;
    second.complete().await?;
    let pinned = dataset.pin(&receipt).await?;
    assert!(dataset.contains(&pinned, quad("old")?).await?);
    assert!(!dataset.contains(&pinned, quad("new")?).await?);
    drop(writer);
    worker.await??;
    Ok(())
}
