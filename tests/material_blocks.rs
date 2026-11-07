use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};
#[tokio::test]
async fn material_blocks_survive_restore_and_publish_semantic_html()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:s> a ga:Scope. <urn:f> a ga:Profile. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer "website";ga:destination "site"."#).await?;
    tokio::fs::write(
        input.path().join("index.md"),
        r#"---
title: Panels
permalink: /
---
# Panels

=== "Business & management"

    A **shared** [detail](detail.md).

    !!! info "Read this first"

        Nested information.

=== "Technical"

    ???+ warning "Review <carefully>"

        Visible technical detail.

    ```text
    === "literal, not a tab"
    ```
"#,
    )
    .await?;
    tokio::fs::write(
        input.path().join("detail.md"),
        "---\ntitle: Detail\n---\n# Detail\n",
    )
    .await?;
    let built =
        website::collect(input.path(), &settings, "urn:p", "urn:t", 2).await?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(input.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publish failed")
    };
    let html =
        tokio::fs::read_to_string(result.directory.join("index.html")).await?;
    assert!(html.contains("class=\"ga-tabs\""), "{html}");
    assert!(html.contains("Read this first"));
    assert!(html.contains("<details"));
    assert!(html.contains("href=\"detail/\""), "{html}");
    assert!(html.contains("literal, not a tab"));
    assert!(!html.contains("grapharch-content"));
    assert!(html.contains("Review &lt;carefully&gt;"));
    Ok(())
}
