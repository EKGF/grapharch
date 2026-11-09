use grapharch::publication::{
    dataset::Dataset,
    render::{PublishOutcome, Publisher},
    showcases,
};

#[tokio::test]
async fn preamble_collector_extracts_only_legal_content_and_rejects_executable_markup()
-> anyhow::Result<()> {
    use grapharch::publication::{
        collect::{Collector, HtmlPreambleFile},
        view::RevisionView,
        writer::GraphWriter,
    };
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("template.html");
    tokio::fs::write(&path, r#"<script>template machinery</script><details class="preamble-section"><summary>Preamble</summary><p>Complete copyright notice.</p><p class="section-title">LICENSES</p><ol><li>Keep all notices.</li></ol></details>{% for cls in classes %}"#).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let collector = HtmlPreambleFile::new(
        "urn:legal:project",
        "urn:legal:collector",
        path.clone(),
        "https://example.org/spec/",
        &["urn:legal:target"],
    )?;
    collector.collect(&writer).await?;
    let revision = dataset.snapshot_for("urn:legal:project").await?;
    let view = RevisionView::new(dataset.clone(), revision.clone());
    let body = view.text(&collector.fragment_id(), "body").await?;
    assert!(body.contains("Complete copyright notice."));
    assert!(body.contains("LICENSES"));
    assert!(body.contains("Keep all notices."));
    assert!(!body.contains("template machinery") && !body.contains("{%"));
    tokio::fs::write(&path, r#"<details class="preamble-section"><summary>Preamble</summary><script>unsafe()</script></details>"#).await?;
    assert!(collector.collect(&writer).await.is_err());
    assert!(dataset.ensure_ready("urn:legal:project").await.is_err());
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn legal_preamble_survives_source_removal_and_has_distinct_html_and_pdf_layouts()
-> anyhow::Result<()> {
    verify_preamble("specification", "LibertinusSerif").await
}

#[tokio::test]
async fn omg_legal_preamble_retains_its_source_and_embeds_the_formal_serif_font()
-> anyhow::Result<()> {
    verify_preamble("omg", "LiberationSerif").await
}

async fn verify_preamble(
    style: &str,
    embedded_font: &str,
) -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("project.ttl");
    let mut source =
        tokio::fs::read_to_string("examples/specification/project.ttl").await?;
    source.push_str(r#"
<urn:specification:target> ga:preamble <urn:example:legal> .
<urn:example:legal> a ga:Fragment ; ga:subject ex:subject ; ga:contribution ex:authored ; dct:title "Legal notices" ;
    ga:format "text/html" ; ga:body "<h2>Copyright and permissions</h2><p>Copyright © 2026 Example Authority.</p><h2>Terms of use</h2><p>Permission is granted subject to the complete terms in this notice.</p><h2>Warranty</h2><p>This specification is provided AS IS.</p>" .
"#);
    source.push_str(&format!("\nex:profile ga:style \"{style}\" .\n"));
    tokio::fs::write(&path, source).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let archive = temp.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(path).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], temp.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let selector =
        scraper::Selector::parse("details.publication-preamble").unwrap();
    let panel = document
        .select(&selector)
        .next()
        .expect("preamble must be an accessible disclosure");
    assert!(
        panel.value().attr("open").is_none(),
        "legal panel is collapsed by default"
    );
    assert!(panel.text().collect::<String>().contains("provided AS IS"));
    let css =
        tokio::fs::read_to_string(publication.directory.join("style.css"))
            .await?;
    assert!(
        css.contains(".publication-preamble>summary"),
        "legal disclosure needs its own visual treatment"
    );
    assert!(css.contains(
        ".legal-content :is(p,li){text-align:justify;text-align-last:left}"
    ));
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("#let ga-legal-notices"));
    assert!(typst.contains(&format!("#let ga-style = \"{style}\"")));
    assert!(
        typst.contains(
            r#"set text(font: if ga-omg { "Liberation Serif" } else { "Libertinus Serif" }, size: 9pt, fill: ga-muted)"#
        ),
        "legal fine print must have distinct readable typography"
    );
    assert!(
        typst.contains("#text(font: \"Liberation Sans\", size: 8.5pt"),
        "legal section labels remain distinct from legal body text"
    );
    assert!(typst.contains("set par(justify: true"));
    assert!(
        typst.find("#ga-legal-notices[").unwrap() <
            typst.find("#outline(").unwrap()
    );
    assert!(typst.contains("provided AS IS"));
    let pdf =
        tokio::fs::read(publication.directory.join("document.pdf")).await?;
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        String::from_utf8_lossy(&pdf).contains(embedded_font),
        "{style} must embed its selected legal-body font {embedded_font}"
    );
    if let Ok(destination) = std::env::var("GRAPHARCH_PREAMBLE_REVIEW") {
        let destination = std::path::Path::new(&destination).join(style);
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "style.css", "main.typ", "document.pdf"] {
            tokio::fs::copy(
                publication.directory.join(name),
                std::path::Path::new(&destination).join(name),
            )
            .await?;
        }
    }
    Ok(())
}
