use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, MarkdownFile},
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
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
        "---\ntitle: Detail\nletter_prefix: A\ngrapharch:\n  fragment: \
         urn:detail\n---\n# Detail\n\nA detail.",
    )
    .await?;
    let config = input.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:s> a ga:Scope. <urn:f> a ga:Profile. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:pdf> a ga:Target;ga:scope <urn:ps>;ga:profile <urn:pf>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>;ga:destination "pdf". <urn:ps> a ga:Scope;ga:include <urn:detail>. <urn:pf> a ga:Profile;ga:placement <urn:placement>. <urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1. <urn:component> a ga:Component;ga:fragment <urn:detail>."#).await?;
    let built =
        website::collect(input.path(), &config, "urn:p", "urn:t", 2).await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf_plan = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:pdf")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
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
    let PublishOutcome::Published(result) = Publisher::new(restored.clone())
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
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .render(&pdf_plan, output.path())
        .await?
    else {
        panic!("PDF failed")
    };
    assert!(pdf.directory.join("document.pdf").exists());
    if std::env::var_os("GRAPHARCH_INCLUDE_PREVIEW").is_some() {
        let destination = std::path::Path::new("output/markdown-includes");
        tokio::fs::create_dir_all(destination).await?;
        tokio::fs::copy(
            pdf.directory.join("document.pdf"),
            destination.join("document.pdf"),
        )
        .await?;
        tokio::fs::write(destination.join("index.html"), html).await?;
    }
    Ok(())
}
#[tokio::test]
async fn includes_fail_on_cycles_and_root_escape() -> Result<()> {
    for directive in ["--8<-- \"index.md\"", "--8<-- \"../outside.md\""] {
        let root = tempfile::tempdir()?;
        let source = root.path().join("index.md");
        tokio::fs::write(
            &source,
            format!("---\ntitle: Safety\n---\n{directive}"),
        )
        .await?;
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
        let error = format!("{:#}", result.unwrap_err());
        assert!(
            error.contains("include cycle") || error.contains("explicit root"),
            "{error}"
        );
        drop(writer);
        worker.await??;
    }
    Ok(())
}

#[tokio::test]
async fn nested_includes_use_explicit_roots_and_track_unselected_source_changes()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let docs = root.path().join("docs");
    tokio::fs::create_dir(&docs).await?;
    let path = docs.join("index.md");
    tokio::fs::write(
        &path,
        "---\ntitle: Root\n---\n# Root\n--8<-- \"nested.md\"\n",
    )
    .await?;
    tokio::fs::write(
        docs.join("nested.md"),
        "{% include-markdown \"shared.md\" start=\"START\" end=\"END\" %}",
    )
    .await?;
    tokio::fs::write(
        root.path().join("shared.md"),
        "beforeSTARTretainedENDafter",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let collector = MarkdownFile::in_site("urn:p", "urn:c", docs, path)?
        .with_include_roots(vec![root.path().to_path_buf()])?;
    collector.collect(&writer).await?;
    let archive = root.path().join("before.nq");
    dataset.export(archive.clone()).await?;
    let before = tokio::fs::read_to_string(&archive).await?;
    assert!(before.contains("retained"));
    tokio::fs::write(
        root.path().join("shared.md"),
        "changedSTARTretainedENDafter",
    )
    .await?;
    collector.collect(&writer).await?;
    dataset.export(archive.clone()).await?;
    let after = tokio::fs::read_to_string(archive).await?;
    assert_ne!(
        before, after,
        "unselected source edits must update provenance"
    );
    assert!(after.contains("changedSTART"));
    drop(writer);
    worker.await??;
    Ok(())
}
