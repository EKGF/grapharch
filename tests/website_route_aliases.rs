use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};
const CONFIG: &str = r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:s> a ga:Scope. <urn:f> a ga:Profile. <urn:section> ga:routeAlias "old/section/"."#;
#[tokio::test]
async fn aliases_redirect_to_retained_page_sections_without_sources()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let out = tempfile::tempdir()?;
    tokio::fs::write(
        input.path().join("index.md"),
        "---\naliases:\n  - legacy/\ngrapharch:\n  fragments:\n    section: \
         urn:section\n---\n# Home\n\n## Section\n\nRetained explanation.\n",
    )
    .await?;
    let config = input.path().join("project.ttl");
    tokio::fs::write(&config, CONFIG).await?;
    let built =
        website::collect(input.path(), &config, "urn:p", "urn:t", 2).await?;
    let archive = out.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(input.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, out.path())
        .await?
    else {
        panic!("site failed")
    };
    let page =
        tokio::fs::read_to_string(site.directory.join("legacy/index.html"))
            .await?;
    assert!(page.contains("href=\"../\""));
    let section = tokio::fs::read_to_string(
        site.directory.join("old/section/index.html"),
    )
    .await?;
    assert!(section.contains("href=\"../../#section\""));
    assert!(section.contains("location.hash"));
    assert!(section.contains("location.replace"));
    Ok(())
}

#[tokio::test]
async fn unsafe_and_colliding_aliases_fail_before_publication() -> Result<()> {
    for (alias, expected) in [
        ("../escape/", "invalid publication route"),
        (
            "https://example.org/",
            "invalid publication route",
        ),
        ("/", "collides"),
        ("style.css/nested/", "collides"),
    ] {
        let input = tempfile::tempdir()?;
        let output = tempfile::tempdir()?;
        tokio::fs::write(
            input.path().join("index.md"),
            format!("---\naliases: [\"{alias}\"]\n---\n# Home\n"),
        )
        .await?;
        let config = input.path().join("project.ttl");
        tokio::fs::write(&config, CONFIG).await?;
        let error =
            match website::collect(input.path(), &config, "urn:p", "urn:t", 2)
                .await
            {
                Err(error) => error,
                Ok(built) => {
                    Publisher::new(built.dataset)
                        .with_edition(
                            grapharch::publication::pdf_name::Edition::Default,
                        )
                        .with_edition(
                            grapharch::publication::pdf_name::Edition::Default,
                        )
                        .render(&built.plan, output.path())
                        .await
                        .err()
                        .expect("unsafe alias must fail")
                },
            };
        assert!(
            format!("{error:#}").contains(expected),
            "{error:#}"
        );
        assert!(
            !output.path().join("site").exists(),
            "failed alias must not publish a partial site"
        );
    }
    Ok(())
}
