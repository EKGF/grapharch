//! The website header shell is generic: organization identity, header links
//! and identity marks come from RDF on the profile or its themes, and a
//! header without them keeps title, sections and controls on one row.
use {
    anyhow::Result,
    base64::{Engine, engine::general_purpose::STANDARD},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::PathBuf,
};

const MARK: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 \
                    10 10\"><circle cx=\"5\" cy=\"5\" r=\"4\" \
                    fill=\"#a30\"/></svg>";

fn definitions(profile: &str) -> String {
    format!(
        r#"@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Records"; ga:target <urn:target>; ga:repository <https://github.com/example-org/records> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer ga:WebsiteRenderer; ga:destination "site" .
<urn:profile> a ga:Profile; ga:layout "documentation"; ga:navigation <urn:primary> {profile} .
<urn:primary> a ga:Navigation; dct:title "Sections"; ga:navigationRegion "primary"; ga:navigationEntry <urn:home-entry> .
<urn:home-entry> a ga:NavigationEntry; ga:order 1; dct:title "Overview"; ga:page <urn:home> .
<urn:brand> a ga:BrandMark; dct:title "Example Org"; ga:wordmark "Example"; ga:url "https://example.org/"; ga:image "data:image/svg+xml;base64,{mark}" .
<urn:umbrella> a ga:BrandMark; dct:title "Umbrella Body"; ga:url "https://umbrella.example/"; ga:order 1; ga:image "https://umbrella.example/logo.svg" .
<urn:about> dct:title "About"; ga:url "https://example.org/about/"; ga:order 1 .
"#,
        mark = STANDARD.encode(MARK)
    )
}

async fn publish(profile: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(&path, definitions(profile)).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Overview\npermalink: /\ngrapharch:\n  subject: \
         urn:home\n---\n# Records\n\nHome.\n",
    )
    .await?;
    let collected = website::collect(
        source.path(),
        &path,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    collected.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&collected.plan, &output.path().join("output"))
        .await?
    else {
        panic!("publication expected")
    };
    let directory = publication.directory.clone();
    Ok((output, directory))
}

fn select(html: &str, selector: &str) -> Vec<String> {
    let document = scraper::Html::parse_document(html);
    document
        .select(&scraper::Selector::parse(selector).unwrap())
        .map(|element| element.html())
        .collect()
}

#[tokio::test]
async fn profile_identity_and_links_fill_the_reader_header() -> Result<()> {
    let (_output, site) = publish(
        "; ga:brandLogo <urn:brand>; ga:identityMark <urn:umbrella>; \
         ga:headerLink <urn:about>",
    )
    .await?;
    let html = tokio::fs::read_to_string(site.join("index.html")).await?;
    let brand = select(&html, ".community-header-row > a.brand-identity");
    assert_eq!(brand.len(), 1);
    assert!(brand[0].contains("aria-label=\"Example Org home\""));
    assert!(brand[0].contains("<span>Example</span>"));
    let src = brand[0]
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        tokio::fs::read_to_string(site.join(src)).await?,
        MARK,
        "the embedded mark is published beside the pages"
    );
    assert_eq!(
        select(
            &html,
            ".community-menu nav.header-navigation[aria-label='Example Org'] a"
        )
        .len(),
        1
    );
    let marks = select(&html, ".site-links a.identity-mark img");
    assert_eq!(marks.len(), 1);
    assert!(marks[0].contains("alt=\"Umbrella Body\""));
    assert!(
        !html.contains("theme-ekgf") && !html.contains("omg.org"),
        "no community identity is implied"
    );
    Ok(())
}

#[tokio::test]
async fn reader_header_without_identity_is_one_row() -> Result<()> {
    let (_output, site) = publish("").await?;
    let html = tokio::fs::read_to_string(site.join("index.html")).await?;
    assert!(select(&html, ".community-header-row").is_empty());
    let row = select(&html, "header .documentation-product-row");
    assert_eq!(row.len(), 1);
    for part in [
        ".documentation-title",
        ".primary-navigation",
        ".site-links .source-link",
    ] {
        assert_eq!(
            select(&row[0], part).len(),
            1,
            "{part} shares the product row"
        );
    }
    Ok(())
}

#[tokio::test]
async fn invalid_brand_marks_fail() -> Result<()> {
    for extra in [
        "; ga:brandLogo <urn:about>",
        "; ga:brandLogo <urn:brand>, <urn:umbrella>",
    ] {
        assert!(publish(extra).await.is_err(), "{extra}");
    }
    let (_output, site) = publish("; ga:brandLogo <urn:broken> . <urn:broken> a ga:BrandMark; dct:title \"X\"; ga:url \"https://x.example/\"; ga:image \"data:text/html;base64,PGI+\"").await.map(|_| unreachable!()).unwrap_or_else(|_| (tempfile::tempdir().unwrap(), PathBuf::new()));
    assert_eq!(
        site,
        PathBuf::new(),
        "an unsupported data URL is rejected"
    );
    Ok(())
}
