//! Theme tokens on websites: a theme refining a website skin overrides its
//! screen values, catalogue themes other than the skins are website themes
//! too, and print tokens are accepted without a website effect.
use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::PathBuf,
};

const DEFINITIONS: &str = r##"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
<urn:project> a ga:Project; dct:title "Example"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer ga:WebsiteRenderer; ga:destination "site" TARGET .
<urn:publisher> a <https://ekgf.org/ontology/artifact-governance#Authority>; rdfs:label "Example Standards Body"; ga:shortName "ESB"; ga:accentColor "#7a1f5c" .
<urn:profile> a ga:Profile; ga:theme THEME .
"##;

struct Site {
    _output:   tempfile::TempDir,
    directory: PathBuf,
}

impl Site {
    async fn publish(theme: &str, target: &str, extra: &str) -> Result<Self> {
        let source = tempfile::tempdir()?;
        let definitions = source.path().join("project.ttl");
        tokio::fs::write(
            &definitions,
            format!(
                "{}\n{extra}\n",
                DEFINITIONS
                    .replace("THEME", theme)
                    .replace("TARGET", target)
            ),
        )
        .await?;
        tokio::fs::write(
            source.path().join("index.md"),
            "---\ntitle: Home\npermalink: /\n---\n# Example\n\nSee \
             [the example](https://example.org/).\n",
        )
        .await?;
        let collected = website::collect(
            source.path(),
            &definitions,
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
        Ok(Self { _output: output, directory })
    }

    async fn css(&self) -> Result<String> {
        Ok(tokio::fs::read_to_string(self.directory.join("style.css")).await?)
    }

    async fn home(&self) -> Result<String> {
        Ok(
            tokio::fs::read_to_string(self.directory.join("index.html"))
                .await?,
        )
    }
}

#[tokio::test]
async fn consumer_theme_tokens_refine_the_reader_website() -> Result<()> {
    let site = Site::publish(
        "<urn:house>",
        "",
        r##"<urn:house> a ga:Theme; ga:baseTheme ga:ReaderTheme;
            ga:accent "#aa3300"; ga:headingColor "accent"; ga:linkColor "#112233";
            ga:tableHeaderFill "accent-tint"; ga:tableGrid true; ga:linkUnderline false;
            ga:bodyFont "Liberation Serif"; ga:heading1Weight "regular";
            ga:paper "us-letter"; ga:heading1Size "30pt"; ga:contentWidth "80rem" ."##,
    )
    .await?;
    let css = site.css().await?;
    assert!(
        css.contains(
            ":root{--accent:#aa3300;--content-width:80rem;--heading:\
             var(--accent);--link:#112233;--table-head:color-mix(in \
             srgb,var(--accent) 6%,var(--paper,#fff))}"
        ),
        "colour tokens, including accent references, become properties"
    );
    for rule in [
        ":root{font-family:\"Liberation Serif\",sans-serif}",
        ":root article h1{font-weight:400}",
        ":root article :is(p,li,td,th,dd)>a{text-decoration:none}",
        ":root article :is(th,td){border:1px solid \
         var(--table-rule,var(--line))}",
    ] {
        assert!(css.contains(rule), "missing {rule}");
    }
    assert!(
        css.contains("h1,h2,h3,h4{color:var(--heading,inherit)"),
        "headings use the heading colour"
    );
    assert!(!css.contains("30pt") && !css.contains("us-letter"));
    Ok(())
}

#[tokio::test]
async fn specification_theme_is_a_website_theme_with_a_status_band()
-> Result<()> {
    let site = Site::publish(
        "ga:SpecificationTheme",
        "; ga:publisher <urn:publisher>; ga:publicationStatus \"Draft\"",
        "",
    )
    .await?;
    let css = site.css().await?;
    assert!(
        css.contains(":root{--accent:#7a1f5c;--heading:var(--accent)}"),
        "the publisher's accent and accent headings apply"
    );
    assert!(css.contains(":root article :is(th,td){border:1px solid"));
    let html = site.home().await?;
    let document = scraper::Html::parse_document(&html);
    let band = document
        .select(
            &scraper::Selector::parse("body[data-status-band] .status-band")
                .unwrap(),
        )
        .map(|band| band.text().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(band, ["ESB Draft"]);
    let without_status = Site::publish("ga:SpecificationTheme", "", "").await?;
    assert!(!without_status.home().await?.contains("status-band"));
    let reader = Site::publish(
        "ga:ReaderTheme",
        "; ga:publisher <urn:publisher>; ga:publicationStatus \"Draft\"",
        "",
    )
    .await?;
    assert!(
        !reader.home().await?.contains("class=\"status-band\""),
        "the reader skin owns its screen values"
    );
    Ok(())
}

#[tokio::test]
async fn every_catalogue_theme_publishes_a_website() -> Result<()> {
    for theme in [
        "ga:ReaderTheme",
        "ga:SpecificationTheme",
        "ga:OverviewTheme",
        "ga:EkgfTheme",
        "ga:EkgfProjectTheme",
    ] {
        Site::publish(theme, "", "").await?;
    }
    Ok(())
}

#[tokio::test]
async fn invalid_screen_tokens_fail() -> Result<()> {
    for declaration in [
        "ga:headingColor \"red\"",
        "ga:tableGrid \"yes\"",
        "ga:bodyFont \"A\\\";}\"",
        "ga:figureCaptionAlign \"justify\"",
        "ga:contentWidth \"80vw\"",
    ] {
        let result = Site::publish(
            "<urn:house>",
            "",
            &format!(
                "<urn:house> a ga:Theme; ga:baseTheme ga:ReaderTheme; \
                 {declaration} ."
            ),
        )
        .await;
        assert!(result.is_err(), "{declaration} must be rejected");
    }
    Ok(())
}
