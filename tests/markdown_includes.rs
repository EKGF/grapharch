use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, MarkdownFile},
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
};
#[tokio::test]
async fn included_prose_and_badges_are_retained_with_dependencies() -> Result<()>
{
    let input = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir(input.path().join("_parts")).await?;
    tokio::fs::write(
        input.path().join("_parts/summary.md"),
        "ignored\n<!--start-->\nIncluded **prose** and a \
         [detail](../detail.md).\n<!--end-->\nignored end",
    )
    .await?;
    tokio::fs::write(
        input.path().join("_parts/nested.md"),
        "--8<-- \"_parts/summary.md\"\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("index.md"),
        r#"---
title: Example
permalink: /
---
# <span class="objective-badge" data-letter="3"></span> Example

{% include-markdown "_parts/summary.md" start="<!--start-->" end="<!--end-->" %}
"#,
    )
    .await?;
    tokio::fs::write(
        input.path().join("detail.md"),
        "---\ntitle: Detail\nletter_prefix: A\n---\n# Detail\n\nA detail.",
    )
    .await?;
    let config = input.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:s> a ga:Scope. <urn:f> a ga:Profile. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer "website";ga:destination "site"."#).await?;
    let built =
        website::collect(input.path(), &config, "urn:p", "urn:t", 2).await?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let graph = tokio::fs::read_to_string(&archive).await?;
    assert!(graph.contains("wasDerivedFrom"));
    assert!(graph.contains("summary.md"));
    assert!(
        graph.contains("ignored end"),
        "full included source provenance must be retained"
    );
    tokio::fs::remove_dir_all(input.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication failed")
    };
    let html =
        tokio::fs::read_to_string(result.directory.join("index.html")).await?;
    assert!(html.contains("Included <strong>prose</strong>"));
    assert!(html.contains("href=\"detail/\""));
    assert!(!html.contains("ignored end"));
    assert!(html.contains("class=\"identifier-badge\""));
    assert!(html.contains("id=\"example\""));
    let detail =
        tokio::fs::read_to_string(result.directory.join("detail/index.html"))
            .await?;
    assert!(detail.contains("class=\"identifier-badge\""));
    assert!(detail.contains("id=\"detail\""));
    Ok(())
}
#[tokio::test]
async fn includes_fail_on_cycles_and_root_escape() -> Result<()> {
    for directive in ["--8<-- \"index.md\"", "--8<-- \"../outside.md\""] {
        let root = tempfile::tempdir()?;
        let source = root.path().join("index.md");
        tokio::fs::write(&source, directive).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) = GraphWriter::spawn(dataset, 4, 1024 * 1024)?;
        let result = MarkdownFile::in_site(
            "urn:p",
            "urn:c",
            root.path().to_path_buf(),
            source,
        )?
        .collect(&writer)
        .await;
        assert!(result.is_err(), "unsafe includes must fail");
        drop(writer);
        worker.await??;
    }
    Ok(())
}
