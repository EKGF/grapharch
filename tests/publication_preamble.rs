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
        oxrdf::NamedNode::new("urn:legal:preamble")?,
    )?;
    collector.collect(&writer).await?;
    let revision = dataset.snapshot_for("urn:legal:project").await?;
    let view = RevisionView::new(dataset.clone(), revision.clone());
    assert_eq!(
        collector.fragment_id().as_str(),
        "urn:legal:preamble"
    );
    let body = view.text(&collector.fragment_id(), "body").await?;
    assert!(
        view.objects_by(
            &oxrdf::NamedNode::new("urn:legal:target")?,
            &grapharch::publication::term("notice"),
        )
        .await?
        .next()
        .await?
        .is_none(),
        "the collector must not choose the targets of its fragment"
    );
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
async fn legal_notice_survives_source_removal_and_has_distinct_html_and_pdf_layouts()
-> anyhow::Result<()> {
    verify_notices("ga:SpecificationTheme", "LibertinusSerif", "").await
}

#[tokio::test]
async fn consumer_theme_notices_retain_their_source_and_embed_the_formal_serif_font()
-> anyhow::Result<()> {
    verify_notices(
        "<urn:example:submission-theme:theme>",
        "LiberationSerif",
        include_str!("fixtures/submission-theme.ttl"),
    )
    .await
}

#[tokio::test]
async fn ordered_notices_put_copyright_holders_first_without_a_fixed_banner()
-> anyhow::Result<()> {
    let typst = render_notices(
        "ga:SpecificationTheme",
        "",
        r#"
<urn:specification:target> ga:notice <urn:example:holders> .
<urn:example:holders> ga:order 1 ;
    ga:copyrightHolder <urn:example:second>, <urn:example:first> .
<urn:example:first> ga:order 1 ; ga:holderName "Adaptive Analytics, Inc." ; ga:copyrightYears "2024-2025" .
<urn:example:second> ga:order 2 ; ga:holderName "Object Management Group, Inc." ; ga:copyrightYears "2022-2026" .
"#,
    )
    .await?
    .1;
    let first = typst
        .find("Copyright © 2024-2025, Adaptive Analytics, Inc.")
        .expect("first holder");
    let second = typst
        .find("Copyright © 2022-2026, Object Management Group, Inc.")
        .expect("second holder");
    let terms = typst.find("Legal notices").expect("legal notice title");
    assert!(
        first < second && second < terms,
        "notices follow ga:order"
    );
    assert!(
        !typst.contains("LEGAL NOTICES"),
        "no engine-supplied banner"
    );
    assert!(
        typst.find("#ga-notices[").unwrap() < typst.find("#outline(").unwrap()
    );
    Ok(())
}

async fn render_notices(
    theme: &str,
    theme_definitions: &str,
    extra: &str,
) -> anyhow::Result<(String, String, Vec<u8>, String)> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("project.ttl");
    let mut source =
        tokio::fs::read_to_string("examples/specification/project.ttl").await?;
    source.push_str(r#"
<urn:specification:target> ga:notice <urn:example:legal-notice> .
<urn:example:legal-notice> ga:order 2 ; ga:fragment <urn:example:legal> .
<urn:example:legal> a ga:Fragment ; ga:subject ex:subject ; ga:contribution ex:authored ; dct:title "Legal notices" ;
    ga:format "text/html" ; ga:body "<h2>Copyright and permissions</h2><p>Copyright © 2026 Example Authority.</p><h2>Terms of use</h2><p>Permission is granted subject to the complete terms in this notice.</p><h2>Warranty</h2><p>This specification is provided AS IS.</p>" .
"#);
    source.push_str(extra);
    source.push_str(theme_definitions);
    source.push_str(&format!("\nex:profile ga:theme {theme} .\n"));
    tokio::fs::write(&path, source).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let archive = temp.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(path).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&plans[0], temp.path())
        .await?
    else {
        panic!("publication required")
    };
    let read = |name: &str| publication.directory.join(name);
    Ok((
        tokio::fs::read_to_string(read("index.html")).await?,
        tokio::fs::read_to_string(read("main.typ")).await?,
        tokio::fs::read(publication.pdf().expect("published PDF edition"))
            .await?,
        tokio::fs::read_to_string(read("style.css")).await?,
    ))
}

async fn verify_notices(
    theme: &str,
    embedded_font: &str,
    theme_definitions: &str,
) -> anyhow::Result<()> {
    let (html, typst, pdf, css) =
        render_notices(theme, theme_definitions, "").await?;
    let document = scraper::Html::parse_document(&html);
    let selector =
        scraper::Selector::parse("details.publication-preamble").unwrap();
    let panel = document
        .select(&selector)
        .next()
        .expect("notice must be an accessible disclosure");
    assert!(
        panel.value().attr("open").is_none(),
        "legal panel is collapsed by default"
    );
    assert!(panel.text().collect::<String>().contains("provided AS IS"));
    assert!(
        css.contains(".publication-preamble>summary"),
        "legal disclosure needs its own visual treatment"
    );
    assert!(css.contains(
        ".legal-content :is(p,li){text-align:justify;text-align-last:left}"
    ));
    assert!(typst.contains("#ga-notice-title[#text(\"Legal notices\")]"));
    assert!(!typst.contains("LEGAL NOTICES"));
    assert!(
        typst.contains(
            "#ga-notice-heading(ga-theme.notice_heading_size)[#text(\"Terms \
             of use\")]"
        ),
        "notice headings take the theme's notice heading style"
    );
    assert!(
        typst.contains(
            "#[\n#show heading: set text(font: ga-theme.toc_title_font"
        )
    );
    assert!(
        typst.find("#ga-notices[").unwrap() < typst.find("#outline(").unwrap()
    );
    assert!(typst.contains("provided AS IS"));
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        String::from_utf8_lossy(&pdf).contains(embedded_font),
        "{theme} must embed its selected notice font {embedded_font}"
    );
    Ok(())
}
