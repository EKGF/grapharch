use {
    super::*,
    oxrdf::{GraphName, Literal},
};

#[tokio::test]
async fn version_comparison_binds_existing_attempt_and_skips_unrelated_versions()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let quad = Quad::new(
        NamedNode::new("urn:subject")?,
        NamedNode::new("urn:predicate")?,
        Literal::new_simple_literal("value"),
        GraphName::DefaultGraph,
    );
    let first = dataset
        .begin("urn:project", "urn:processor", "source", "r1")
        .await?;
    let first_id = first.id.clone();
    first.push(vec![quad.clone()]).await?;
    first.complete().await?;
    let first_snapshot = dataset.snapshot_for("urn:project").await?;
    let equal = dataset
        .begin("urn:project", "urn:processor", "source", "r1")
        .await?;
    equal.push(vec![quad.clone()]).await?;
    let inspection = equal.clone();
    dataset
        .run(move |store| {
            store.transaction::<_, io::Error>(|transaction| {
                let previous = inspection
                    .previous_data(&transaction)?
                    .expect("existing version");
                assert_eq!(previous, first_id);
                let query = inspection.changed_query(&previous);
                assert!(query.contains(&previous.to_string()));
                assert!(query.contains(&inspection.id.to_string()));
                assert!(
                    !query.contains("?previous"),
                    "old attempt must be bound before comparing sets"
                );
                assert!(matches!(
                    transaction.query(&query)?,
                    QueryResults::Boolean(false)
                ));
                Ok(())
            })?;
            Ok(())
        })
        .await?;
    equal.complete().await?;
    assert_eq!(
        first_snapshot,
        dataset.snapshot_for("urn:project").await?
    );
    // An existing contribution elsewhere must not trigger a cold version's
    // set comparison, regardless of its source identity or physical graphs.
    let cold = dataset
        .begin("urn:project", "urn:other", "unrelated", "r1")
        .await?;
    let inspection = cold.clone();
    dataset
        .run(move |store| {
            store.transaction::<_, io::Error>(|transaction| {
                assert!(inspection.previous_data(&transaction)?.is_none());
                Ok(())
            })?;
            Ok(())
        })
        .await?;
    cold.push(vec![quad]).await?;
    cold.complete().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_or_conflicting_version_data_pointers_are_rejected()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let attempt = dataset
        .begin("urn:project", "urn:processor", "source", "r1")
        .await?;
    dataset
        .run(move |store| {
            store.transaction::<_, io::Error>(|mut transaction| {
                let invalid = Quad::new(
                    attempt.version.clone(),
                    term("data"),
                    Literal::new_simple_literal("not an IRI"),
                    NamedNode::new_unchecked(CONTROL),
                );
                transaction.insert(invalid.as_ref())?;
                assert!(attempt.previous_data(&transaction).is_err());
                transaction.remove(invalid.as_ref())?;
                for id in ["urn:previous:one", "urn:previous:two"] {
                    transaction.insert(
                        Quad::new(
                            attempt.version.clone(),
                            term("data"),
                            NamedNode::new_unchecked(id),
                            NamedNode::new_unchecked(CONTROL),
                        )
                        .as_ref(),
                    )?;
                }
                assert!(attempt.previous_data(&transaction).is_err());
                Ok(())
            })?;
            Ok(())
        })
        .await?;
    Ok(())
}
