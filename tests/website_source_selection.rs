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
<urn:project> a ga:Project; ga:target <urn:target>; ga:excludeSourcePath "generated" .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
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
