use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};
#[tokio::test]
async fn image_alternatives_survive_source_removal_and_print_selects_light()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    for (name, color) in [("light.svg", "blue"), ("dark.svg", "red")] {
        tokio::fs::write(source.path().join(name), format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><rect width="120" height="60" fill="{color}"/></svg>"#)).await?;
    }
    tokio::fs::write(
        source.path().join("index.md"),
        "# Image alternatives\n\n![Light model](light.svg \"Original \
         title\"){.only-light}\n\n![Dark \
         model](dark.svg){.only-dark}\n\nRetained explanation.",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:pdf> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>;ga:destination "pdf". <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
    let built =
        website::collect(source.path(), &config, "urn:p", "urn:t", 2).await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:pdf")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored.clone())
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("HTML failed")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(html.contains("Light model"));
    assert!(html.contains("Dark model"));
    assert!(html.contains("data-image-scheme=\"light\""));
    assert!(html.contains("data-image-scheme=\"dark\""));
    assert!(html.contains("title=\"Original title\""));
    assert!(!html.contains("grapharch:image"));
    let PublishOutcome::Published(pdf) =
        Publisher::new(restored).render(&pdf, output.path()).await?
    else {
        panic!("PDF failed")
    };
    let typst =
        tokio::fs::read_to_string(pdf.directory.join("main.typ")).await?;
    assert!(typst.contains("Light model"));
    assert!(!typst.contains("Dark model"));
    assert!(typst.contains("Retained explanation"));
    if std::env::var_os("GRAPHARCH_IMAGE_VARIANT_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/image-variants").await?;
        for file in &site.manifest.files {
            let destination =
                std::path::Path::new("output/image-variants").join(file);
            if let Some(parent) = destination.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(site.directory.join(file), destination).await?;
        }
        tokio::fs::copy(
            pdf.directory.join("document.pdf"),
            "output/image-variants/document.pdf",
        )
        .await?;
    }
    Ok(())
}
