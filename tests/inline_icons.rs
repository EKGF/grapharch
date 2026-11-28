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
struct IconPublication {
    output:   tempfile::TempDir,
    restored: Dataset,
    site:     grapharch::publication::plan::ResolvedPlan,
    pdf:      grapharch::publication::plan::ResolvedPlan,
}

/// Collect the icon fixture, export its KG and restore it without sources.
async fn icon_publication(profile: &str) -> Result<IconPublication> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("index.md"),
        "# Phase navigation\n\n:material-cached:{ .lg } \
         **Deploy**\n\n[:octicons-arrow-right-24: Learn \
         more](#phase-navigation)\n\n:material-floor-plan: Plan phase and \
         :material-run: Run phase.",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config,format!(r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf". <urn:s> a ga:Scope. <urn:f> a ga:Profile{profile}."#)).await?;
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
    Ok(IconPublication { output, restored, site: built.plan, pdf })
}

#[tokio::test]
async fn decorative_icons_survive_source_removal_in_html_and_pdf() -> Result<()>
{
    let IconPublication { output, restored, site, pdf } =
        icon_publication("").await?;
    let PublishOutcome::Published(site) = Publisher::new(restored.clone())
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&site, output.path())
        .await?
    else {
        panic!("HTML failed")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert_eq!(
        html.matches("aria-hidden=\"true\" focusable=\"false\"")
            .count(),
        4
    );
    assert!(!html.contains(":material-"));
    assert!(html.contains("Learn more"));
    assert!(site.directory.join("grapharch-icon-licenses.txt").exists());
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    publisher.render(&pdf, output.path()).await?;
    let PublishOutcome::Published(pdf) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF failed")
    };
    let typst = tokio::fs::read_to_string(pdf.typst_source()).await?;
    assert!(typst.contains("#image(bytes("));
    assert!(typst.contains("Deploy"));
    assert!(typst.contains("Run phase"));
    assert!(!typst.contains(":material-"));
    if std::env::var_os("GRAPHARCH_ICON_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/inline-icons").await?;
        tokio::fs::copy(
            pdf.pdf().expect("published PDF edition"),
            "output/inline-icons/document.pdf",
        )
        .await?;
        tokio::fs::write("output/inline-icons/index.html", html).await?;
        tokio::fs::copy(
            site.directory.join("style.css"),
            "output/inline-icons/style.css",
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn decorative_icons_are_pdf_artifacts_in_pdf_ua_documents() -> Result<()>
{
    let IconPublication { output, restored, pdf, .. } =
        icon_publication(";ga:pdfStandard ga:PdfUa1").await?;
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&pdf, output.path())
        .await?
    else {
        panic!("PDF failed")
    };
    let typst = tokio::fs::read_to_string(pdf.typst_source()).await?;
    assert!(
        typst.contains("#box[#pdf.artifact[#image(bytes("),
        "decorative icons carry no meaning for assistive technology"
    );
    let bytes =
        tokio::fs::read(pdf.pdf().expect("published PDF edition")).await?;
    let part = b"<pdfuaid:part>1</pdfuaid:part>";
    assert!(bytes.windows(part.len()).any(|window| window == part));
    Ok(())
}

/// Every Material for MkDocs icon set resolves from the bundle, attribute
/// lists set the size and alignment, and an unknown shortcode stays visible
/// with a typed warning instead of disappearing.
#[tokio::test]
async fn bundled_icon_sets_attribute_lists_and_unknown_icon_warnings()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("index.md"),
        "# Icons\n\n:material-account-group-outline:{ .lg .middle } \
         Teams\n\n:fontawesome-brands-github: :fontawesome-solid-house: \
         :fontawesome-regular-heart: :simple-rust: :octicons-arrow-right-24:{ \
         .xl }\n\n:material-no-such-icon: stays\n\n:simple-rust:{ .heart } \
         custom class\n",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(
        &config,
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer ga:WebsiteRenderer;ga:destination "site". <urn:s> a ga:Scope. <urn:f> a ga:Profile."#,
    )
    .await?;
    let built =
        website::collect(source.path(), &config, "urn:p", "urn:t", 2).await?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let graph = tokio::fs::read_to_string(&archive).await?;
    assert!(
        graph.contains("\"icon-unknown\""),
        "unknown icon diagnostic"
    );
    assert!(graph.contains("material-no-such-icon"));
    assert!(graph.contains("\"icon-attribute-ignored\""));
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("HTML failed")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    let document = scraper::Html::parse_document(&html);
    let icons = scraper::Selector::parse("svg.inline-icon").unwrap();
    let icons = document.select(&icons).collect::<Vec<_>>();
    assert_eq!(icons.len(), 7, "{html}");
    for icon in &icons {
        assert_eq!(icon.attr("fill"), Some("currentColor"));
        assert_eq!(icon.attr("aria-hidden"), Some("true"));
        assert!(icon.attr("viewBox").is_some());
    }
    let first = icons[0].attr("class").unwrap();
    assert!(
        first.contains("inline-icon-lg") &&
            first.contains("inline-icon-middle")
    );
    assert!(html.contains("inline-icon-xl"));
    assert!(
        html.contains(":material-no-such-icon:"),
        "unknown stays visible"
    );
    assert!(!html.contains(":simple-rust:"));
    assert!(!html.contains("{ .lg .middle }") && !html.contains("{ .heart }"));
    let licenses = tokio::fs::read_to_string(
        site.directory.join("grapharch-icon-licenses.txt"),
    )
    .await?;
    for notice in ["Font Awesome", "Octicons", "Simple Icons", "Pictogrammers"]
    {
        assert!(licenses.contains(notice), "{notice}");
    }
    Ok(())
}
