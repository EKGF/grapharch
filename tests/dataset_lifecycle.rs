use {
    grapharch::publication::dataset::Dataset,
    oxigraph::model::{GraphName, Literal, NamedNode, Quad},
};

fn quad(value: &str) -> anyhow::Result<Quad> {
    Ok(Quad::new(
        NamedNode::new("urn:subject")?,
        NamedNode::new("urn:predicate")?,
        Literal::new_simple_literal(value),
        NamedNode::new("urn:original-graph")?,
    ))
}

#[tokio::test]
async fn completion_is_atomic_and_failed_attempt_preserves_previous_content()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let first = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source-a",
            "revision-1",
        )
        .await?;
    first.push(vec![quad("old")?]).await?;
    assert_eq!(dataset.active_count().await?, 0);
    first.complete().await?;
    assert_eq!(dataset.active_count().await?, 1);
    let failed = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source-a",
            "revision-2",
        )
        .await?;
    failed.push(vec![quad("incomplete")?]).await?;
    failed.abort().await?;
    assert_eq!(dataset.active_count().await?, 1);
    let snapshot = dataset.snapshot().await?;
    assert!(dataset.contains(&snapshot, quad("old")?).await?);
    assert!(!dataset.contains(&snapshot, quad("incomplete")?).await?);
    Ok(())
}

#[tokio::test]
async fn restart_reopens_published_content_and_preserves_named_graphs()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let snapshot;
    {
        let dataset = Dataset::open(dir.path().to_owned()).await?;
        let contribution = dataset
            .begin(
                "urn:project:test",
                "urn:collector:a",
                "source-a",
                "r1",
            )
            .await?;
        contribution.push(vec![quad("persisted")?]).await?;
        contribution.complete().await?;
        snapshot = dataset.snapshot().await?;
    }
    let reopened = Dataset::open(dir.path().to_owned()).await?;
    assert!(reopened.contains(&snapshot, quad("persisted")?).await?);
    Ok(())
}

#[tokio::test]
async fn superseded_attempt_cannot_complete_and_pinned_revision_stays_unchanged()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let old = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source-a",
            "r1",
        )
        .await?;
    old.push(vec![quad("one")?]).await?;
    old.complete().await?;
    let pinned = dataset.snapshot().await?;
    let stale = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source-a",
            "r2",
        )
        .await?;
    let current = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source-a",
            "r3",
        )
        .await?;
    assert!(stale.complete().await.is_err());
    current.push(vec![quad("three")?]).await?;
    current.complete().await?;
    assert!(dataset.contains(&pinned, quad("one")?).await?);
    let new = dataset.snapshot().await?;
    assert!(dataset.contains(&new, quad("three")?).await?);
    assert!(!dataset.contains(&new, quad("one")?).await?);
    Ok(())
}

#[tokio::test]
async fn repeat_completion_is_idempotent_and_source_ownership_is_separate()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    for collector in ["urn:collector:a", "urn:collector:b"] {
        let attempt = dataset
            .begin("urn:project:test", collector, "source", "r1")
            .await?;
        attempt.push(vec![quad(collector)?]).await?;
        attempt.complete().await?;
    }
    let revision = dataset.snapshot().await?;
    let same = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    same.push(vec![quad("urn:collector:a")?]).await?;
    same.complete().await?;
    assert_eq!(revision, dataset.snapshot().await?);
    assert_eq!(dataset.active_count().await?, 2);
    Ok(())
}

#[tokio::test]
async fn oversized_chunk_is_rejected_before_writing() -> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    let huge = "x".repeat(2 * 1024 * 1024);
    assert!(attempt.push(vec![quad(&huge)?]).await.is_err());
    assert_eq!(dataset.active_count().await?, 0);
    assert_eq!(
        quad("x")?.graph_name,
        GraphName::NamedNode(NamedNode::new("urn:original-graph")?)
    );
    Ok(())
}

#[tokio::test]
async fn same_declared_revision_with_different_content_is_rejected()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let first = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    first.push(vec![quad("first")?]).await?;
    first.complete().await?;
    let changed = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    changed.push(vec![quad("different")?]).await?;
    assert!(changed.complete().await.is_err());
    let snapshot = dataset.snapshot().await?;
    assert!(dataset.contains(&snapshot, quad("first")?).await?);
    Ok(())
}

#[tokio::test]
async fn empty_replacement_removes_old_assertions() -> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let first = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    first.push(vec![quad("first")?]).await?;
    first.complete().await?;
    dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r2",
        )
        .await?
        .complete()
        .await?;
    assert_eq!(dataset.active_count().await?, 0);
    Ok(())
}

#[tokio::test]
async fn projects_using_the_same_collector_and_source_are_isolated()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    for project in ["urn:project:a", "urn:project:b"] {
        let attempt = dataset
            .begin(project, "urn:collector:shared", "source", "r1")
            .await?;
        attempt.push(vec![quad(project)?]).await?;
        attempt.complete().await?;
    }
    assert_eq!(dataset.active_count().await?, 2);
    Ok(())
}

#[tokio::test]
async fn exported_dataset_can_resume_without_original_sources()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    attempt.push(vec![quad("portable")?]).await?;
    attempt.complete().await?;
    let pinned = dataset.snapshot().await?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("dataset.nq");
    dataset.export(path.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(path).await?;
    assert!(restored.contains(&pinned, quad("portable")?).await?);
    Ok(())
}

#[tokio::test]
async fn failed_import_rolls_back_and_preserves_unrelated_graphs()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    attempt.push(vec![quad("safe")?]).await?;
    attempt.complete().await?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("broken.nq");
    tokio::fs::write(&path, "<urn:s> <urn:p> <urn:o> <urn:g> .\nBROKEN")
        .await?;
    assert!(dataset.import(path).await.is_err());
    assert_eq!(dataset.active_count().await?, 1);
    Ok(())
}

#[tokio::test]
async fn default_graph_and_named_graph_cannot_alias() -> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    let mut default = quad("default only")?;
    default.graph_name = GraphName::DefaultGraph;
    attempt.push(vec![default.clone()]).await?;
    attempt.complete().await?;
    let snapshot = dataset.snapshot().await?;
    default.graph_name =
        GraphName::NamedNode(NamedNode::new("urn:grapharch:default")?);
    assert!(!dataset.contains(&snapshot, default).await?);
    Ok(())
}

#[tokio::test]
async fn independent_assembly_can_detect_pending_and_failed_inputs()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r1",
        )
        .await?;
    assert!(dataset.ensure_ready("urn:project:test").await.is_err());
    attempt.abort().await?;
    assert!(dataset.ensure_ready("urn:project:test").await.is_err());
    dataset
        .begin(
            "urn:project:test",
            "urn:collector:a",
            "source",
            "r2",
        )
        .await?
        .complete()
        .await?;
    dataset.ensure_ready("urn:project:test").await?;
    Ok(())
}
