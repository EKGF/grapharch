use {
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        scope::ScopeResolver,
        term,
        writer::GraphWriter,
    },
    oxrdf::{NamedNode, Quad},
};

#[tokio::test]
async fn generic_scopes_have_expected_members_and_exclusions()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        "urn:project:fixtures",
        "urn:collector:definitions",
        "tests/fixtures/publication/projects.ttl".into(),
    )?
    .collect(&writer)
    .await?;
    let input = dataset.snapshot().await?;
    for (scope, included, excluded) in [
        (
            "family",
            &[
                "ontology-a",
                "ontology-b",
                "class-a",
                "class-b",
                "shared-term",
            ][..],
            &["internal", "external-standard"][..],
        ),
        (
            "use-cases",
            &["root-use-case", "child-use-case"][..],
            &["fund-a"][..],
        ),
        ("portfolio", &["fund-a"][..], &["fund-b"][..]),
    ] {
        let scope = NamedNode::new(format!("urn:example:{scope}"))?;
        let resolver = ScopeResolver::new(dataset.clone(), input.clone());
        let plan = resolver
            .resolve("urn:project:fixtures", &scope, &writer)
            .await?;
        let output = plan.snapshot.clone();
        let plan = plan.id;
        for member in included {
            assert!(
                dataset
                    .contains(
                        &output,
                        Quad::new(
                            plan.clone(),
                            term("member"),
                            NamedNode::new(format!("urn:example:{member}"))?,
                            plan.clone()
                        )
                    )
                    .await?,
                "missing {member}"
            );
        }
        for member in excluded {
            assert!(
                !dataset
                    .contains(
                        &output,
                        Quad::new(
                            plan.clone(),
                            term("member"),
                            NamedNode::new(format!("urn:example:{member}"))?,
                            plan.clone()
                        )
                    )
                    .await?,
                "unexpected {member}"
            );
        }
    }
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn traversal_respects_depth_instead_of_following_an_entire_chain()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("scopes.ttl");
    tokio::fs::write(
        &path,
        "@prefix ga: <https://ekgf.org/ontology/grapharch#> . <urn:scope> a \
         ga:Scope ; ga:include <urn:a> ; ga:traverse <urn:child> ; \
         ga:maxDepth 1 . <urn:a> <urn:child> <urn:b> . <urn:b> <urn:child> \
         <urn:c> .",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:project", "urn:collector", path)?
        .collect(&writer)
        .await?;
    let snapshot = dataset.snapshot().await?;
    let plan = ScopeResolver::new(dataset.clone(), snapshot)
        .resolve(
            "urn:project",
            &NamedNode::new("urn:scope")?,
            &writer,
        )
        .await?;
    let output = plan.snapshot.clone();
    let plan = plan.id;
    assert!(
        dataset
            .contains(
                &output,
                Quad::new(
                    plan.clone(),
                    term("member"),
                    NamedNode::new("urn:b")?,
                    plan.clone()
                )
            )
            .await?
    );
    assert!(
        !dataset
            .contains(
                &output,
                Quad::new(
                    plan.clone(),
                    term("member"),
                    NamedNode::new("urn:c")?,
                    plan
                )
            )
            .await?
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn cyclic_scope_composition_fails_without_publishing_a_partial_plan()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("scopes.ttl");
    tokio::fs::write(
        &path,
        "@prefix ga: <https://ekgf.org/ontology/grapharch#> . <urn:a> a \
         ga:Scope ; ga:compose <urn:b> . <urn:b> a ga:Scope ; ga:compose \
         <urn:a> .",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:project", "urn:collector", path)?
        .collect(&writer)
        .await?;
    let before = dataset.snapshot().await?;
    let result = ScopeResolver::new(dataset.clone(), before.clone())
        .resolve("urn:project", &NamedNode::new("urn:a")?, &writer)
        .await;
    assert!(result.unwrap_err().to_string().contains("cyclic"));
    assert_eq!(before, dataset.snapshot().await?);
    drop(writer);
    worker.await??;
    Ok(())
}
