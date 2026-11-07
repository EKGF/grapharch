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
async fn authored_topology_survives_source_removal_without_duplicate_pages()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("docs/use-case/customer"))
        .await?;
    tokio::fs::write(source.path().join("docs/index.md"), "# Catalog\n")
        .await?;
    tokio::fs::write(
        source.path().join("docs/use-case/customer/index.md"),
        "---\ntitle: Customer\nsummary: 'A **source-owned** \
         summary.'\nis-part-of: []\n---\n\nUnique canonical customer prose.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/use-case/customer/identity.md"),
        "---\ntitle: Identity\nsummary: 'Verify identity.'\nis-part-of: \
         ['..']\nis-used-in: []\n---\n\nUnique canonical identity prose.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/use-case/evidence.md"),
        "---\ntitle: Evidence\nsummary: Reusable source \
         evidence.\nis-part-of: []\nis-used-in: \
         ['/customer']\n---\n\nCanonical evidence prose.\n",
    )
    .await?;
    let definitions = source.path().join("grapharch.ttl");
    tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:contentSource "docs";ga:collector <urn:cases>;ga:target <urn:t>, <urn:pdf>. <urn:pdf> a ga:Target;ga:renderer "typst";ga:destination "pdf";ga:scope <urn:s>;ga:profile <urn:f>. <urn:cases> a ga:Collector;ga:kind "authored-use-cases";ga:source "docs/use-case". <urn:t> a ga:Target;ga:renderer "website";ga:destination "site";ga:scope <urn:s>;ga:profile <urn:f>. <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
    let built =
        website::collect(source.path(), &definitions, "urn:p", "urn:t", 2)
            .await?;
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
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored.clone())
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("website failed")
    };
    let customer = tokio::fs::read_to_string(
        site.directory.join("use-case/customer/index.html"),
    )
    .await?;
    let identity = tokio::fs::read_to_string(
        site.directory.join("use-case/customer/identity/index.html"),
    )
    .await?;
    assert_eq!(
        customer.matches("Unique canonical customer prose.").count(),
        1
    );
    assert_eq!(
        identity.matches("Unique canonical identity prose.").count(),
        1
    );
    assert!(customer.contains("Use-case relationships"));
    assert!(
        customer.contains("<object class=\"interactive-svg\""),
        "native use-case diagram must retain clickable node labels"
    );
    let projected = site
        .manifest
        .files
        .iter()
        .find(|path| path.starts_with("assets/interactive-"))
        .expect("retained projected diagram");
    let svg = tokio::fs::read_to_string(site.directory.join(projected)).await?;
    assert!(svg.contains("target=\"_top\""));
    assert!(
        svg.contains("prefers-color-scheme: dark"),
        "native interactive labels need an authored dark palette"
    );
    assert!(svg.contains("../use-case/customer/"));
    assert!(identity.contains("Ownership parent"));
    assert!(
        identity.contains("../../../use-case/customer/#use-case-relationships"),
        "{}",
        identity
            .split("Ownership parent")
            .nth(1)
            .unwrap_or("missing parent")
            .chars()
            .take(700)
            .collect::<String>()
    );
    assert!(customer.contains("source-owned"));
    assert!(customer.contains("Reused components"));
    assert!(customer.contains("Evidence"));
    assert!(
        site.manifest
            .files
            .iter()
            .any(|file| file.ends_with(".svg"))
    );
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .render(&pdf_plan, output.path())
        .await?
    else {
        panic!("PDF failed")
    };
    let typst =
        tokio::fs::read_to_string(pdf.directory.join("main.typ")).await?;
    assert!(typst.contains("Unique canonical customer prose."));
    assert!(typst.contains("Use-case relationships"));
    assert!(
        tokio::fs::metadata(pdf.directory.join("document.pdf"))
            .await?
            .len() >
            1000
    );
    Ok(())
}

#[tokio::test]
async fn invalid_authored_relationships_fail_with_source_evidence() -> Result<()>
{
    for (parent, expected) in [
        ("/missing", "unresolved isPartOf"),
        ("/child", "cycle"),
        (
            "..",
            "independent roots require explicit is-part-of: []",
        ),
    ] {
        let source = tempfile::tempdir()?;
        tokio::fs::create_dir_all(source.path().join("use-case/root")).await?;
        tokio::fs::write(source.path().join("index.md"), "# Catalog").await?;
        tokio::fs::write(
            source.path().join("use-case/root/index.md"),
            format!(
                "---\ntitle: Root\nsummary: Root summary.\nis-part-of: \
                 ['{parent}']\n---\n\nRoot prose."
            ),
        )
        .await?;
        tokio::fs::write(
            source.path().join("use-case/child.md"),
            "---\ntitle: Child\nsummary: Child summary.\nis-part-of: \
             ['/root']\n---\n\nChild prose.",
        )
        .await?;
        let config = source.path().join("project.ttl");
        tokio::fs::write(&config, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:collector <urn:c>;ga:target <urn:t>. <urn:c> a ga:Collector;ga:kind "authored-use-cases";ga:source "use-case". <urn:t> a ga:Target;ga:renderer "website";ga:destination "site";ga:scope <urn:s>;ga:profile <urn:f>. <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
        let error =
            match website::collect(source.path(), &config, "urn:p", "urn:t", 2)
                .await
            {
                Ok(_) => panic!("invalid topology accepted"),
                Err(error) => format!("{error:#}"),
            };
        assert!(error.contains(expected), "{error}");
        if expected.contains("unresolved") {
            assert!(
                error.contains("use-case/root/index.md"),
                "{error}"
            );
        }
    }
    Ok(())
}
