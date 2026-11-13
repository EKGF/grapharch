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
async fn authored_figures_preserve_captions_and_images_after_source_removal()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("image.svg"),r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><rect width="120" height="60" fill="#4960b7"/></svg>"##).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "# Figure review\n\n<figure markdown>\n  ![A diagram](image.svg){ \
         width=\"700\" }\n  <figcaption>Figure 1: Shared model<br/>© Original \
         **contributors**</figcaption>\n</figure>\n\n<figure markdown>\n| \
         Model | Value |\n|---|---|\n| Shared | Useful |\n<figcaption>Model \
         comparison</figcaption>\n</figure>\n",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf". <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
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
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("HTML failed")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(html.contains("<figure class=\"ga-figure\""));
    assert!(html.contains("<figcaption>"));
    assert!(html.contains("Model comparison"));
    assert!(html.contains("<table>"));
    assert!(html.contains("© Original <strong>contributors</strong>"));
    assert!(html.contains("--figure-width:700px"));
    assert!(!html.contains("{ width="));
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&pdf, output.path())
        .await?
    else {
        panic!("PDF failed")
    };
    if std::env::var_os("GRAPHARCH_FIGURE_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/semantic-figures").await?;
        tokio::fs::copy(
            pdf.pdf().expect("published PDF edition"),
            "output/semantic-figures/document.pdf",
        )
        .await?;
        tokio::fs::write("output/semantic-figures/index.html", html).await?;
    }
    Ok(())
}
