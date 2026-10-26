#[path = "support/recorded_images.rs"]
mod recorded_images;

use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, MarkdownFile},
        dataset::Dataset,
        writer::GraphWriter,
    },
    oxrdf::Term,
};

#[tokio::test]
async fn regular_markdown_records_deduplicable_image_inputs_without_fetching_them()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    for name in ["first", "second"] {
        let path = source.path().join(format!("{name}.md"));
        std::fs::write(&path, format!("---\ntitle: {name}\ngrapharch:\n  subject: urn:page:{name}\n---\n# {name}\n\n![Shared badge](https://example.org/badge.svg)\n"))?;
        MarkdownFile::new(
            "urn:test:project",
            &format!("urn:collector:{name}"),
            path,
        )?
        .collect(&writer)
        .await?;
    }
    let revision = dataset.snapshot_for("urn:test:project").await?;
    for name in ["first", "second"] {
        MarkdownFile::new(
            "urn:test:project",
            &format!("urn:collector:{name}"),
            source.path().join(format!("{name}.md")),
        )?
        .collect(&writer)
        .await?;
    }
    assert_eq!(
        revision,
        dataset.snapshot_for("urn:test:project").await?,
        "repeated Markdown collection must preserve its source revision"
    );
    let mut rows = dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT DISTINCT \
             ?fragment ?asset ?url WHERE { GRAPH ?g { ?fragment ga:reference \
             ?ref ; ga:asset ?asset . ?ref ga:asset ?asset ; ga:url ?url } } \
             ORDER BY ?fragment"
                .into(),
        )
        .await?;
    let mut assets = std::collections::HashSet::new();
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        assert_eq!(
            row["url"],
            oxrdf::Literal::new_simple_literal("https://example.org/badge.svg")
                .into()
        );
        let Term::NamedNode(asset) = &row["asset"] else {
            panic!("asset identity required")
        };
        assets.insert(asset.clone());
        count += 1;
    }
    assert_eq!(
        count, 2,
        "each ordinary Markdown source must retain its image request metadata"
    );
    assert_eq!(
        assets.len(),
        1,
        "same URL must have one project-wide image identity"
    );
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let recorded = tempfile::tempdir()?;
    let work =
        grapharch::publication::collect::http_image_collectors_with_factory(
            &dataset,
            "urn:test:project",
            &|project, request| {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                recorded_images::collector(recorded.path(), project, request)
            },
        )
        .await?;
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "two pages must create one specialized image collector input"
    );
    assert_eq!(work.len(), 1);
    grapharch::publication::collect::collect_all(work, &writer, 2).await?;
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn two_regular_markdown_pages_publish_shared_recorded_image_from_restored_kg()
-> Result<()> {
    use grapharch::publication::{
        assembly::Assembler,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
    };
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    std::fs::write(
        &definitions,
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:test:project> a ga:Project ; dct:title "Recorded Markdown sources" ; ga:target <urn:test:site>, <urn:test:pdf> .
<urn:test:scope> a ga:Scope . <urn:test:profile> a ga:Profile ; ga:style "reader" .
<urn:test:site> a ga:Target ; dct:title "Recorded Markdown website" ; ga:scope <urn:test:scope> ; ga:profile <urn:test:profile> ; ga:renderer "website" ; ga:destination "site" .
<urn:test:pdf> a ga:Target ; dct:title "Recorded Markdown review" ; ga:scope <urn:test:scope> ; ga:profile <urn:test:profile> ; ga:renderer "typst" ; ga:destination "review" .
"#,
    )?;
    for (name, route) in [("Home", "/"), ("Second", "/second/")] {
        std::fs::write(source.path().join(format!("{name}.md")), format!("---\ntitle: {name}\npermalink: {route}\ngrapharch:\n  subject: urn:page:{name}\n---\n# {name}\n\n![Shared recorded badge](https://example.org/badge.svg)\n"))?;
    }
    let built = recorded_images::collect_site(
        source.path(),
        &definitions,
        "urn:test:project",
        "urn:test:site",
        2,
    )
    .await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:test:project")
        .await?;
    let pdf = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve(
            "urn:test:project",
            &oxrdf::NamedNode::new("urn:test:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored);
    let PublishOutcome::Published(site) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("site required")
    };
    let images: Vec<_> = site
        .manifest
        .files
        .iter()
        .filter(|path| path.starts_with("assets/"))
        .collect();
    assert_eq!(
        images.len(),
        1,
        "shared URL must have one emitted file"
    );
    for path in ["index.html", "second/index.html"] {
        let html = std::fs::read_to_string(site.directory.join(path))?;
        assert!(html.contains("alt=\"Shared recorded badge\""));
        assert!(!html.contains("src=\"https://example.org/badge.svg\""));
        assert!(
            html.contains(images[0]),
            "page must reference the retained image route"
        );
    }
    let PublishOutcome::Published(review) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF required")
    };
    let typst = std::fs::read_to_string(review.directory.join("main.typ"))?;
    assert_eq!(typst.matches("#image(").count(), 2);
    assert_eq!(
        std::fs::read(site.directory.join(images[0]))?,
        std::fs::read(review.directory.join(images[0]))?
    );
    assert!(
        std::fs::read(review.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn regular_markdown_rejects_non_https_images_before_request_factory_runs()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("unsafe.md");
    std::fs::write(
        &path,
        "![Unsafe image](http://example.org/unsafe.svg)",
    )?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let error =
        MarkdownFile::new("urn:test:project", "urn:test:collector", path)?
            .collect(&writer)
            .await
            .expect_err("unsupported image scheme must fail collection");
    assert!(format!("{error:#}").contains("credential-free HTTPS"));
    drop(writer);
    worker.await??;
    Ok(())
}
