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
async fn footnotes_survive_source_removal_with_linked_html_and_native_pdf_notes()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ngrapharch:\n  fragment: urn:evidence\n---\n# Evidence\n\nFirst \
         claim[^study]. The same evidence[^study] supports another \
         claim.\n\n## Details\n\nAnother observation[^other].\n\n[^study]: A \
         **published study** with [details](#details).\n[^other]: Another \
         retained explanation.\n",
    )
    .await?;
    tokio::fs::write(source.path().join("tab.md"), "# Tabbed notes\n\n=== \"Evidence\"\n\n    Tabbed claim[^study].\n\n    [^study]: A distinct tabbed source note.\n").await?;
    tokio::fs::write(
        source.path().join("note-source.md"),
        "# Source\n\n<!--start-->\nAn included \
         claim[^source].\n<!--end-->\n\nExcluded narrative.\n\n[^source]: \
         Evidence retained outside the selected excerpt.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("excerpt.md"),
        "# Excerpt\n\n{% include-markdown \"note-source.md\" \
         start=\"<!--start-->\" end=\"<!--end-->\" %}\n",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;ga:scope <urn:ps>;ga:profile <urn:pf>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf". <urn:ps> a ga:Scope;ga:include <urn:evidence>. <urn:pf> a ga:Profile;ga:placement <urn:placement>. <urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1. <urn:component> a ga:Component;ga:fragment <urn:evidence>. <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
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
    assert!(!html.contains("[^study]"));
    assert!(!html.contains("[^other]"));
    assert_eq!(
        html.matches("class=\"footnote-reference\"").count(),
        3
    );
    assert_eq!(
        html.matches("class=\"footnote-definition\"").count(),
        2
    );
    assert!(html.contains("published study"));
    assert!(html.contains("Another retained explanation"));
    let tab = tokio::fs::read_to_string(site.directory.join("tab/index.html"))
        .await?;
    assert!(tab.contains("footnote-reference"));
    assert!(
        !tab.contains("id=\"study\""),
        "tab note must have a source-qualified identity"
    );
    assert!(tab.contains("A distinct tabbed source note"));
    let excerpt =
        tokio::fs::read_to_string(site.directory.join("excerpt/index.html"))
            .await?;
    assert!(excerpt.contains("Evidence retained outside the selected excerpt"));
    assert!(!excerpt.contains("Excluded narrative"));
    assert!(excerpt.contains("footnote-reference"));
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    publisher.render(&pdf, output.path()).await?;
    let PublishOutcome::Published(pdf) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF failed")
    };
    let typst = tokio::fs::read_to_string(pdf.typst_source()).await?;
    assert_eq!(typst.matches("#footnote[").count(), 2);
    assert!(
        !typst.contains("#footnote[#par["),
        "note marker and first text must share a line"
    );
    assert_eq!(typst.matches("#footnote(label(").count(), 1);
    assert!(typst.contains("published study"));
    assert!(typst.contains("Another retained explanation"));
    assert!(!typst.contains("[^study]"));
    if std::env::var_os("GRAPHARCH_FOOTNOTE_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/footnotes").await?;
        tokio::fs::copy(
            pdf.pdf().expect("published PDF edition"),
            "output/footnotes/document.pdf",
        )
        .await?;
        tokio::fs::write("output/footnotes/index.html", html).await?;
        tokio::fs::copy(
            site.directory.join("style.css"),
            "output/footnotes/style.css",
        )
        .await?;
    }
    Ok(())
}
