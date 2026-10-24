use {
    grapharch::publication::{dataset::Dataset, writer::GraphWriter},
    oxigraph::model::{Literal, NamedNode, Quad},
};

fn chunk(i: usize) -> anyhow::Result<Vec<Quad>> {
    Ok(vec![Quad::new(
        NamedNode::new(format!("urn:subject:{i}"))?,
        NamedNode::new("urn:predicate")?,
        Literal::from(i as u64),
        oxigraph::model::GraphName::DefaultGraph,
    )])
}

#[tokio::test]
async fn parallel_producers_stream_to_one_bounded_writer() -> anyhow::Result<()>
{
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..8 {
        let writer = writer.clone();
        tasks.spawn(async move {
            let mut stream = writer
                .begin(
                    "urn:project:test",
                    &format!("urn:collector:{i}"),
                    "source",
                    "r1",
                )
                .await?;
            stream.push(chunk(i)?).await?;
            stream.complete().await
        });
    }
    while let Some(result) = tasks.join_next().await {
        result??;
    }
    drop(writer);
    worker.await??;
    assert_eq!(dataset.active_count().await?, 8);
    Ok(())
}

#[tokio::test]
async fn dead_receiver_returns_error_without_hanging() -> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset, 1, 1024 * 1024)?;
    let mut stream = writer
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    worker.abort();
    let _ = worker.await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        stream.push(chunk(0)?),
    )
    .await?;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn invalid_queue_configuration_fails_fast() -> anyhow::Result<()> {
    assert!(GraphWriter::spawn(Dataset::memory()?, 0, 1024).is_err());
    assert!(GraphWriter::spawn(Dataset::memory()?, 1, 0).is_err());
    Ok(())
}

#[tokio::test]
async fn contribution_metadata_obeys_the_writer_byte_budget()
-> anyhow::Result<()> {
    let (writer, worker) = GraphWriter::spawn(Dataset::memory()?, 1, 128)?;
    assert!(
        writer
            .begin(
                "urn:project",
                "urn:collector",
                &"x".repeat(512),
                "r1"
            )
            .await
            .is_err()
    );
    drop(writer);
    worker.await??;
    Ok(())
}
