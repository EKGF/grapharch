use {anyhow::Result, grapharch::publication::website};

#[tokio::test]
async fn authored_exclusions_are_applied_before_source_budget() -> Result<()> {
    let root = tempfile::tempdir()?;
    tokio::fs::create_dir(root.path().join("generated")).await?;
    for index in 0..260 {
        tokio::fs::write(
            root.path().join(format!("generated/{index}.md")),
            "# Generated\n",
        )
        .await?;
    }
    tokio::fs::write(
        root.path().join("index.md"),
        "# Authoritative page\n",
    )
    .await?;
    let defs = root.path().join("project.ttl");
    let config = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:project> a ga:Project; ga:target <urn:target>; ga:excludeSourcePath "generated", "future-output" .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile .
"#;
    tokio::fs::write(&defs, config).await?;
    let built =
        website::collect(root.path(), &defs, "urn:project", "urn:target", 2)
            .await?;
    let archive = root.path().join("output.nq");
    built.dataset.export(archive.clone()).await?;
    let retained = tokio::fs::read_to_string(archive).await?;
    assert!(!retained.contains("# Generated"));
    assert!(retained.contains("excludeSourcePath"));
    tokio::fs::write(
        &defs,
        config.replace("\"generated\"", "\"../escape\""),
    )
    .await?;
    let error =
        website::collect(root.path(), &defs, "urn:project", "urn:target", 2)
            .await
            .err()
            .expect("unsafe exclusion rejected");
    assert!(
        error.to_string().contains("relative"),
        "{error:#}"
    );
    Ok(())
}

#[tokio::test]
async fn content_root_is_independent_from_project_dependencies() -> Result<()> {
    let root = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir(root.path().join("docs")).await?;
    tokio::fs::create_dir(root.path().join("metadata")).await?;
    tokio::fs::write(
        root.path().join("README.md"),
        "# Repository administration\n",
    )
    .await?;
    tokio::fs::write(
        root.path().join("docs/index.md"),
        "# Product guide\n\n--8<-- \"shared.md\"\n",
    )
    .await?;
    tokio::fs::write(
        root.path().join("metadata/model.ttl"),
        "<urn:model> <urn:label> \"Model source\" .",
    )
    .await?;
    tokio::fs::write(
        root.path().join("shared.md"),
        "Shared authored explanation.\n",
    )
    .await?;
    let definitions = root.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:project> a ga:Project; ga:contentSource "docs"; ga:includeRoot "."; ga:target <urn:target>; ga:collector <urn:nav>, <urn:rdf> .
<urn:nav> a ga:CollectorUse; ga:collectorType ga:DocumentationNavigationCollector; ga:source "docs"; ga:navigationProfile <urn:profile> .
<urn:rdf> a ga:CollectorUse; ga:collectorType ga:RdfFileCollector; ga:source "metadata/model.ttl" .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile .
"#).await?;
    let built = website::collect(
        root.path(),
        &definitions,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let archive = output.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    root.close()?;
    let dataset = grapharch::publication::dataset::Dataset::memory()?;
    dataset.import(archive).await?;
    let grapharch::publication::render::PublishOutcome::Published(result) =
        grapharch::publication::render::Publisher::new(dataset)
            .render(&built.plan, output.path())
            .await?
    else {
        panic!("published")
    };
    let home =
        tokio::fs::read_to_string(result.directory.join("index.html")).await?;
    assert!(home.contains("Product guide"));
    assert!(home.contains("Shared authored explanation."));
    assert!(!home.contains("Repository administration"));
    assert!(
        !tokio::fs::try_exists(result.directory.join("docs/index.html"))
            .await?
    );
    Ok(())
}
