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
};

#[tokio::test]
async fn explicit_anchors_survive_source_deletion_inside_tabs() -> Result<()> {
    let input = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        input.path().join("index.md"),
        "# Home\n\n[Ontology](detail.md#ontology)\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("detail.md"),
        "---\ngrapharch:\n  fragment: urn:detail\n---\n# Detail\n\n=== \
         \"Overview\"\n\n    Ordinary prose.\n\n=== \"Ontology\"\n\n    <span \
         id=\"ontology\"></span>\n\n    Semantic definition.\n\n    \
         [Return](#ontology)\n",
    )
    .await?;
    let config = input.path().join("project.ttl");
    tokio::fs::write(&config, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:s> a ga:Scope. <urn:f> a ga:Profile. <urn:pdf> a ga:Target;ga:scope <urn:ps>;ga:profile <urn:pf>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>;ga:destination "pdf". <urn:ps> a ga:Scope;ga:include <urn:detail>. <urn:pf> a ga:Profile;ga:placement <urn:placement>. <urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1. <urn:component> a ga:Component;ga:fragment <urn:detail>."#).await?;
    let built =
        website::collect(input.path(), &config, "urn:p", "urn:t", 2).await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf_plan = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve(
            "urn:p",
            &oxrdf::NamedNode::new("urn:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(input.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored.clone())
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("site failed")
    };
    let home =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    let detail =
        tokio::fs::read_to_string(site.directory.join("detail/index.html"))
            .await?;
    assert!(home.contains("href=\"detail/#ontology\""));
    assert_eq!(detail.matches("id=\"ontology\"").count(), 1);
    assert!(detail.contains("href=\"#ontology\""));
    assert!(!detail.contains("urn:grapharch:anchor:"));
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .render(&pdf_plan, output.path())
        .await?
    else {
        panic!("pdf failed")
    };
    assert!(pdf.directory.join("document.pdf").exists());
    Ok(())
}
