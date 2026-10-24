use {
    grapharch::publication::{dataset::Dataset, term},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

#[tokio::test]
async fn logical_queries_join_assertions_across_collectors_in_one_pinned_revision()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    for (collector, predicate, value) in [
        ("urn:collector:a", "first", "one"),
        ("urn:collector:b", "second", "two"),
    ] {
        let attempt = dataset
            .begin("urn:project", collector, "source", "r1")
            .await?;
        attempt
            .push(vec![Quad::new(
                NamedNode::new("urn:subject")?,
                term(predicate),
                Literal::new_simple_literal(value),
                GraphName::DefaultGraph,
            )])
            .await?;
        attempt.complete().await?;
    }
    let snapshot = dataset.snapshot().await?;
    let first = Dataset::logical_pattern(
        &snapshot,
        &format!("?s {} ?first", term("first")),
        0,
    );
    let second = Dataset::logical_pattern(
        &snapshot,
        &format!("?s {} ?second", term("second")),
        1,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?s ?first ?second WHERE {{ {first} {second} }}"
        ))
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing result"))?;
    assert_eq!(row["s"], NamedNode::new("urn:subject")?.into());
    assert!(rows.next().await?.is_none());
    Ok(())
}

#[tokio::test]
async fn empty_snapshot_is_valid_and_repeatable() -> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    assert_eq!(
        dataset.snapshot().await?,
        dataset.snapshot().await?
    );
    Ok(())
}
