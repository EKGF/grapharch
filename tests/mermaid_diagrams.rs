use grapharch::publication::{
    dataset::Dataset,
    render::{PublishOutcome, Publisher},
    website,
};

#[path = "support/definitions.rs"]
mod definitions;

#[tokio::test]
async fn a_documented_mermaid_fragment_is_shared_by_specification_html_and_pdf()
-> anyhow::Result<()> {
    use grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        document::{Documentor, MermaidDocumentor},
        plan::TargetPlanner,
        writer::GraphWriter,
    };
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let definitions = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:intro-body",
        "https://ekgf.org/ontology/grapharch#body",
        definitions::literal(
            "## Pipeline\n\n```mermaid\nflowchart LR\n A[Collect] --> \
             B[Publish]\n```\n",
        ),
    )?;
    tokio::fs::write(&path, definitions).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    RdfFile::new(
        "urn:specification:project",
        "urn:specification:collector",
        path.clone(),
    )?
    .collect(&writer)
    .await?;
    MermaidDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:specification:project").await?,
        "urn:specification:project",
    )?
    .document(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &oxrdf::NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let out = tempfile::tempdir()?;
    let archive = out.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(path).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) =
        Publisher::new(restored).render(&plan, out.path()).await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("alt=\"Documentation diagram\""));
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("#image(\"assets/image-"));
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

const DEFINITIONS: &str = r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:diagram:project> a ga:Project ; dct:title "Diagrams" ; ga:scope <urn:diagram:scope> ; ga:target <urn:diagram:site> .
<urn:diagram:scope> a ga:Scope .
<urn:diagram:profile> a ga:Profile .
<urn:diagram:site> a ga:Target ; dct:title "Diagrams" ; ga:scope <urn:diagram:scope> ; ga:profile <urn:diagram:profile> ; ga:renderer "website" ; ga:destination "site" .
"#;

#[tokio::test]
async fn unsupported_or_malformed_mermaid_fails_collection()
-> anyhow::Result<()> {
    for code in [
        "flowchart LR\n A[unclosed",
        "sequenceDiagram\n Alice->>Bob: Hello",
        "flowchart LR\n A --> B\n click A \"https://example.org\"",
    ] {
        let source = tempfile::tempdir()?;
        tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS)
            .await?;
        tokio::fs::write(
            source.path().join("index.md"),
            format!(
                "---\ntitle: Diagram\ngrapharch:\n  subject: \
                 urn:diagram:architecture\n---\n# \
                 Diagram\n\n```mermaid\n{code}\n```\n"
            ),
        )
        .await?;
        assert!(
            website::collect(
                source.path(),
                &source.path().join("project.ttl"),
                "urn:diagram:project",
                "urn:diagram:site",
                2
            )
            .await
            .is_err(),
            "unsupported or unsafe Mermaid must fail: {code}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn removing_a_fence_retires_its_generated_assets_and_references()
-> anyhow::Result<()> {
    use grapharch::publication::{
        collect::{Collector, MarkdownFile},
        document::{Documentor, MermaidDocumentor},
        view::RevisionView,
        writer::GraphWriter,
    };
    let source = tempfile::tempdir()?;
    let path = source.path().join("index.md");
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let collector = MarkdownFile::new(
        "urn:diagram:project",
        "urn:diagram:collector",
        path.clone(),
    )?;
    tokio::fs::write(
        &path,
        "# Pipeline\n\n```mermaid\nflowchart LR\n A --> B\n```\n",
    )
    .await?;
    collector.collect(&writer).await?;
    MermaidDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
        "urn:diagram:project",
    )?
    .document(&writer)
    .await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
    );
    assert!(
        view.objects(&collector.fragment_id(), "asset")
            .await?
            .next()
            .await?
            .is_some()
    );
    tokio::fs::write(&path, "# Pipeline\n\nNo diagram.\n").await?;
    collector.collect(&writer).await?;
    MermaidDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
        "urn:diagram:project",
    )?
    .document(&writer)
    .await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
    );
    assert!(
        view.objects(&collector.fragment_id(), "asset")
            .await?
            .next()
            .await?
            .is_none()
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn mermaid_becomes_a_retained_svg_and_renders_without_original_sources()
-> anyhow::Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Architecture\ngrapharch:\n  subject: \
         urn:diagram:architecture\n---\n# \
         Architecture\n\n```mermaid\nflowchart LR\n C[Collect] --> \
         D[(Dataset)]\n D --> R[Render]\n```\n",
    )
    .await?;
    let built = website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:diagram:project",
        "urn:diagram:site",
        2,
    )
    .await?;
    let out = tempfile::tempdir()?;
    let archive = out.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, out.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(!html.contains("language-mermaid"));
    let document = scraper::Html::parse_document(&html);
    let selector = scraper::Selector::parse("article img").unwrap();
    let image = document
        .select(&selector)
        .next()
        .expect("diagram must be an image");
    let svg = tokio::fs::read_to_string(
        publication
            .directory
            .join(image.value().attr("src").unwrap()),
    )
    .await?;
    assert!(
        svg.contains("<svg") &&
            svg.contains("Collect") &&
            svg.contains("Dataset")
    );
    assert!(!svg.contains("<script") && !svg.contains("foreignObject"));
    Ok(())
}
