use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

const CONFIG: &str = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Method"; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:profile> a ga:Profile; ga:style "ekgf"; ga:layout "documentation"; ga:navigation <urn:tabs>, <urn:sidebar> .
<urn:tabs> a ga:Navigation; dct:title "Sections"; ga:navigationRegion "primary"; ga:navigationEntry <urn:start>, <urn:learn> .
<urn:start> a ga:NavigationEntry; ga:page <urn:home>; ga:order 1; dct:title "Introduction" .
<urn:learn> a ga:NavigationEntry; ga:page <urn:guide>; ga:order 2; dct:title "Learn" .
<urn:sidebar> a ga:Navigation; dct:title "Method guide"; ga:navigationRegion "sidebar"; ga:navigationEntry <urn:group>, <urn:guide-entry>, <urn:again>, <urn:external> .
<urn:group> a ga:NavigationEntry; dct:title "Concepts"; ga:order 1 .
<urn:guide-entry> a ga:NavigationEntry; ga:page <urn:guide>; ga:parentEntry <urn:group>; ga:order 1; dct:title "Use cases" .
<urn:again> a ga:NavigationEntry; ga:page <urn:guide>; ga:order 2; dct:title "Quick reference"; ga:headingAnchor "details" .
<urn:external> a ga:NavigationEntry; ga:url "https://example.org/"; ga:order 3; dct:title "Elsewhere" .
"#;

async fn render(
    config: &str,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let source = tempfile::tempdir()?;
    let out = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, config).await?;
    for (path, title, id, body) in [
        (
            "index.md",
            "Home",
            "urn:home",
            "# Home\n\nWelcome.",
        ),
        (
            "guide.md",
            "Guide",
            "urn:guide",
            "# Guide\n\n## Details\n\nUseful details.",
        ),
    ] {
        tokio::fs::write(
            source.path().join(path),
            format!(
                "---\ntitle: {title}\nnav_exclude: true\ngrapharch:\n  \
                 subject: {id}\n---\n{body}\n"
            ),
        )
        .await?;
    }
    let built = website::collect(
        source.path(),
        &definitions,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let archive = out.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let destination = if config == CONFIG {
        std::env::var_os("GRAPHARCH_NAV_REVIEW")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| out.path().to_owned())
    } else {
        out.path().to_owned()
    };
    let PublishOutcome::Published(result) = Publisher::new(restored)
        .render(&built.plan, &destination)
        .await?
    else {
        panic!("published")
    };
    Ok((out, result.directory))
}

#[tokio::test]
async fn independent_navigation_trees_survive_source_removal() -> Result<()> {
    let (_out, site) = render(CONFIG).await?;
    let html = tokio::fs::read_to_string(site.join("guide/index.html")).await?;
    let document = scraper::Html::parse_document(&html);
    let links = |selector: &str| {
        document
            .select(&scraper::Selector::parse(selector).unwrap())
            .map(|e| {
                (
                    e.text().collect::<String>(),
                    e.value().attr("href").unwrap_or("").to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(links(".primary-navigation a"), vec![
        ("Introduction".into(), "../".into()),
        ("Learn".into(), "../guide/".into())
    ]);
    let sidebar = links(".site-navigation a");
    assert_eq!(sidebar, vec![
        ("Use cases".into(), "../guide/".into()),
        (
            "Quick reference".into(),
            "../guide/#details".into()
        ),
        ("Elsewhere".into(), "https://example.org/".into())
    ]);
    assert!(html.contains("data-layout=\"documentation\""));
    assert!(html.contains("Method guide"));
    assert!(!html.contains("brand-version"));
    assert!(site.join("provenance.html").exists());
    Ok(())
}

#[tokio::test]
async fn navigation_rejects_cycles_unselected_pages_and_bad_anchors()
-> Result<()> {
    for (replace, with, expected) in [
        (
            "dct:title \"Concepts\"; ga:order 1",
            "dct:title \"Concepts\"; ga:parentEntry <urn:guide-entry>; \
             ga:order 1",
            "cycle",
        ),
        (
            "ga:page <urn:guide>",
            "ga:page <urn:absent>",
            "not selected",
        ),
        (
            "ga:headingAnchor \"details\"",
            "ga:headingAnchor \"missing\"",
            "anchor",
        ),
    ] {
        let result = render(&CONFIG.replace(replace, with)).await;
        assert!(result.is_err(), "expected {expected}");
        assert!(format!("{:#}", result.err().unwrap()).contains(expected));
    }
    Ok(())
}
