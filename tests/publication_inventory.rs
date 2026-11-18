use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

/// Render the inventory fixture from a restored KG; return the output root.
async fn publish() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Project\npermalink: /\ngrapharch:\n  subject: \
         urn:home\n---\n# Project\n\n[Specification \
         versions](spec-versions.md) [All versions](urn:versions-page) \
         [Versions again](https://example.org/dprod/spec-versions/) \
         [Concept](https://example.org/dprod/spec/#shape) \
         [Elsewhere](https://example.org/about/)\n",
    )
    .await?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions,r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix dcat: <http://www.w3.org/ns/dcat#> .
<urn:project> a ga:Project; dct:title "Project"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile; ga:theme ga:EkgfProjectTheme .
<urn:target> a ga:Target; dct:title "Project"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site"; ga:thisVersion <https://example.org/dprod/> .
<urn:inventory-job> a ga:DocumentorUse; ga:project <urn:project>; ga:documentorType ga:PublicationInventoryDocumentor; ga:subject <urn:inventory>; ga:fragment <urn:versions-page>; ga:route "spec-versions/"; ga:sourcePath "spec-versions.md"; ga:navigationOrder 6; dct:title "Specification versions"; ga:eyebrow "Specification" .
<urn:versions-page> dct:description "Every published version." .
<urn:inventory> dct:modified "2026-11-02T15:00:00Z"; dct:hasPart <urn:main>,<urn:develop>,<urn:preview>,<urn:hidden> .
<urn:main> a doap:Version; ga:order 1; dct:title "DPROD 1.0"; dct:identifier "main"; doap:branch "main"; ga:route "/dprod/spec/main/"; dcat:accessURL <https://example.org/dprod/spec/main/>; ga:advertised true; ga:publicationStatus "Frozen · Normative"; dct:description "The frozen release."; ga:iconName "archive"; ga:iconTone "accent" .
<urn:develop> a doap:Version; ga:order 2; dct:title "Develop (latest)"; dct:identifier "develop"; doap:branch "develop"; dcat:accessURL <https://example.org/dprod/spec/develop/>; ga:advertised true .
<urn:preview> a doap:Version; dct:title "ballot/4"; dct:identifier "ballot-4"; doap:branch "ballot/4"; ga:publicationStatus "Unstandardized pull request preview"; dcat:accessURL <https://preview.example.org/dprod/spec/>; ga:advertised true .
<urn:hidden> a doap:Version; dct:title "merged-branch"; dct:identifier "merged-branch"; doap:branch "merged-branch"; dcat:accessURL <https://old.example.org/dprod/spec/>; ga:advertised false .
"#).await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let export = tempfile::tempdir()?;
    collected
        .dataset
        .export(export.path().join("kg.nq"))
        .await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(export.path().join("kg.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok((output, publication.directory))
}

#[tokio::test]
async fn publication_inventory_is_a_pinned_generated_page_and_route_manifest()
-> Result<()> {
    let (_output, directory) = publish().await?;
    let html =
        tokio::fs::read_to_string(directory.join("spec-versions/index.html"))
            .await?;
    assert!(
        html.contains("DPROD 1.0") &&
            html.contains("Develop (latest)") &&
            html.contains("ballot/4")
    );
    assert!(!html.contains("merged-branch"));
    assert!(html.contains("Unstandardized pull request preview"));
    let document = scraper::Html::parse_document(&html);
    let hero = document
        .select(&scraper::Selector::parse(".project-hero").unwrap())
        .next()
        .unwrap();
    let hero_text: String = hero.text().collect();
    assert!(
        !hero_text.contains("Inventory"),
        "Observation metadata does not belong in the hero"
    );
    assert!(
        !html.contains("remain routable"),
        "Hosting implementation details do not belong in reader content"
    );
    assert!(
        html.contains("href=\"../spec/main/\""),
        "the assembled publication must link to its own spec, not the \
         production host"
    );
    assert!(
        html.find("DPROD 1.0").unwrap() <
            html.find("Develop (latest)").unwrap()
    );
    assert!(
        html.find("Develop (latest)").unwrap() < html.find("ballot/4").unwrap()
    );
    let manifest =
        tokio::fs::read_to_string(directory.join("publication-versions.json"))
            .await?;
    assert!(
        manifest.contains("merged-branch") &&
            manifest.contains("old.example.org")
    );
    assert!(manifest.contains("\"advertised\": false"));
    assert!(
        manifest
            .contains("\"status\": \"Unstandardized pull request preview\"")
    );
    Ok(())
}

/// The hero already names the page and provenance records the observation,
/// so the version cards follow the introduction directly; each whole card is
/// one link named by its title.
#[tokio::test]
async fn version_cards_are_whole_card_links_without_repeated_chrome()
-> Result<()> {
    let (_output, directory) = publish().await?;
    let html =
        tokio::fs::read_to_string(directory.join("spec-versions/index.html"))
            .await?;
    let home = tokio::fs::read_to_string(directory.join("index.html")).await?;
    assert!(
        home.contains("href=\"spec-versions/\""),
        "a link to the page IRI resolves to the relative versions page"
    );
    for removed in ["Available versions", "Last checked", "Open specification"]
    {
        assert!(
            !html.contains(removed),
            "{removed} must not be rendered"
        );
    }
    let document = scraper::Html::parse_document(&html);
    let card = scraper::Selector::parse(
        ".project-section .project-card.project-card-linked",
    )
    .unwrap();
    let link = scraper::Selector::parse("a").unwrap();
    let title = scraper::Selector::parse("h3 > a").unwrap();
    let cards: Vec<_> = document.select(&card).collect();
    assert_eq!(
        cards.len(),
        3,
        "one linked card per advertised version"
    );
    for card in &cards {
        assert_eq!(card.select(&link).count(), 1, "one link per card");
        assert_eq!(
            card.select(&title).count(),
            1,
            "the title is the link"
        );
    }
    let first = cards[0].select(&title).next().unwrap();
    assert_eq!(first.value().attr("href"), Some("../spec/main/"));
    assert_eq!(first.text().collect::<String>(), "DPROD 1.0");
    let style = tokio::fs::read_to_string(directory.join("style.css")).await?;
    assert!(
        style.contains(".project-card-linked h3 a::after") &&
            style.contains(".project-card-linked:has(a:focus-visible)"),
        "the title link stretches over the card and shows a focus ring"
    );
    Ok(())
}

/// The website's own location is metadata: canonical and Open Graph URLs and
/// the sitemap use it, but no link inside the website does.
#[tokio::test]
async fn the_website_location_never_becomes_an_internal_link() -> Result<()> {
    let (_output, directory) = publish().await?;
    let base = "https://example.org/dprod/";
    let href = scraper::Selector::parse("[href]").unwrap();
    let mut pages = 0;
    let mut canonical = 0;
    for page in ["index.html", "spec-versions/index.html"] {
        let html = tokio::fs::read_to_string(directory.join(page)).await?;
        pages += 1;
        for element in scraper::Html::parse_document(&html).select(&href) {
            let value = element.value().attr("href").unwrap();
            if element.value().name() == "link" &&
                element.value().attr("rel") == Some("canonical")
            {
                assert!(
                    value.starts_with(base),
                    "{page}: canonical {value}"
                );
                canonical += 1;
                continue;
            }
            assert!(
                !value.starts_with(base) && !value.starts_with("/dprod/"),
                "{page}: internal link {value} must be relative"
            );
        }
    }
    assert_eq!((pages, canonical), (2, 2));
    let home = tokio::fs::read_to_string(directory.join("index.html")).await?;
    for href in [
        "href=\"spec-versions/\">Versions again",
        "href=\"spec/#shape\">Concept",
        "href=\"https://example.org/about/\">Elsewhere",
    ] {
        assert!(home.contains(href), "missing {href}");
    }
    let sitemap =
        tokio::fs::read_to_string(directory.join("sitemap.xml")).await?;
    assert!(sitemap.contains("https://example.org/dprod/spec-versions/"));
    Ok(())
}

/// Version presentation is inventory data: the engine names no statuses.
#[tokio::test]
async fn versions_carry_their_own_badge_description_icon_and_location()
-> Result<()> {
    let (_output, directory) = publish().await?;
    let html =
        tokio::fs::read_to_string(directory.join("spec-versions/index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let text = |selector: &str| {
        document
            .select(&scraper::Selector::parse(selector).unwrap())
            .map(|element| element.text().collect::<String>())
            .collect::<Vec<_>>()
    };
    assert_eq!(text(".project-hero p:first-child"), [
        "Specification"
    ]);
    assert!(
        text(".project-hero p").contains(&"Every published version.".into())
    );
    assert!(!html.contains("Select a published specification"));
    assert_eq!(text(".project-badge.project-badge-accent"), [
        "Frozen · Normative"
    ]);
    assert_eq!(text(".project-card-icon-accent").len(), 1);
    assert!(html.contains("The frozen release."));
    let meta = text("p.project-card-meta");
    assert_eq!(meta[0], "/dprod/spec/main/");
    assert_eq!(
        meta[2],
        "https://preview.example.org/dprod/spec/ · branch ballot/4 · \
         Unstandardized pull request preview"
    );
    assert_eq!(
        text(".project-section-resources .project-card-linked").len(),
        3
    );
    Ok(())
}
