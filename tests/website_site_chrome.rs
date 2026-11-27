//! Source-owned website chrome: an authored footer, opt-outs for the project
//! page bar and site search, and consumer theme colour tokens.
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
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
<urn:project> a ga:Project; dct:title "Example"; ga:scope <urn:scope>; ga:target <urn:target>; ga:repository <https://github.com/EKGF/example> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer ga:WebsiteRenderer; ga:destination "site" .
<urn:theme> a ga:Theme; ga:baseTheme ga:EkgfProjectTheme; ga:accent "#4051b5"; ga:inkColor "#333333" .
<urn:profile> a ga:Profile; ga:theme <urn:theme>; PROFILE .
<urn:footer> a ga:SiteFooter; ga:footerGroup <urn:links>, <urn:about>; ga:footerNote <urn:note2>, <urn:note1> .
<urn:about> dct:title "Example"; ga:order 1; dct:description "The example project." .
<urn:links> dct:title "Links"; ga:order 2; ga:footerLink <urn:omg>, <urn:spec> .
<urn:spec> dct:title "Specification"; ga:url "spec-versions/"; ga:order 1 .
<urn:omg> dct:title "OMG"; ga:url "https://omg.org"; ga:order 2 .
<urn:note1> ga:order 1; dct:description "Copyright © 2026 Example, Inc." .
<urn:note2> ga:order 2; dct:description "A standard of the [Object Management Group (OMG)](https://omg.org)" .
"##;

async fn publish(profile: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(
        &definitions,
        DEFINITIONS.replace("PROFILE", profile),
    )
    .await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\nnav_order: 1\n---\n# \
         Example\n\nHome.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("why.md"),
        "---\ntitle: Why\npermalink: /why/\nnav_order: 2\n---\n# Why\n\nWhy.\n",
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
    Ok((output, directory))
}

fn select<'a>(
    document: &'a scraper::Html,
    selector: &str,
) -> Vec<scraper::ElementRef<'a>> {
    document
        .select(&scraper::Selector::parse(selector).unwrap())
        .collect()
}

fn text(element: &scraper::ElementRef<'_>) -> String {
    element.text().collect::<String>()
}

#[tokio::test]
async fn authored_footer_replaces_theme_footer_and_opt_outs_apply() -> Result<()>
{
    let (_output, directory) = publish(
        "ga:siteFooter <urn:footer>; ga:projectNavigation false; \
         ga:siteSearch false",
    )
    .await?;
    for route in ["index.html", "why/index.html"] {
        let html = tokio::fs::read_to_string(directory.join(route)).await?;
        let document = scraper::Html::parse_document(&html);
        assert_eq!(
            select(&document, "footer.site-footer").len(),
            1,
            "{route}: the authored footer replaces the theme footer"
        );
        assert!(!html.contains("About EKGF"), "{route}");
        assert!(select(&document, "nav.project-navigation").is_empty());
        assert!(select(&document, ".site-search-open").is_empty());
        assert!(select(&document, "dialog.site-search").is_empty());
        let groups = select(&document, "footer.site-footer .site-footer-group");
        assert_eq!(
            groups
                .iter()
                .map(|group| text(&select_in(group, "h2")[0]))
                .collect::<Vec<_>>(),
            ["Example", "Links"]
        );
        assert_eq!(
            text(&select_in(&groups[0], "p")[0]),
            "The example project."
        );
        let links = select_in(&groups[1], "a");
        assert_eq!(links.iter().map(text).collect::<Vec<_>>(), [
            "Specification",
            "OMG"
        ]);
        let base = url::Url::parse(&format!("https://example.org/{route}"))?;
        assert_eq!(
            base.join(links[0].value().attr("href").unwrap())?.as_str(),
            "https://example.org/spec-versions/"
        );
        let notes = select(
            &document,
            "footer.site-footer .site-footer-notes p",
        );
        assert_eq!(text(&notes[0]), "Copyright © 2026 Example, Inc.");
        assert_eq!(
            text(&notes[1]),
            "A standard of the Object Management Group (OMG)"
        );
        assert_eq!(
            select_in(&notes[1], "a")[0].value().attr("href"),
            Some("https://omg.org")
        );
        assert!(
            html.contains("Generated by"),
            "the generator credit remains in the authored footer"
        );
    }
    assert!(!directory.join("search-index.json").exists());
    let css = tokio::fs::read_to_string(directory.join("style.css")).await?;
    assert!(
        css.contains(".theme-ekgf{--accent:#4051b5;--ink:#333333}"),
        "consumer theme tokens become website colour properties"
    );
    Ok(())
}

fn select_in<'a>(
    element: &scraper::ElementRef<'a>,
    selector: &str,
) -> Vec<scraper::ElementRef<'a>> {
    element
        .select(&scraper::Selector::parse(selector).unwrap())
        .collect()
}

#[tokio::test]
async fn website_chrome_defaults_are_unchanged() -> Result<()> {
    let (_output, directory) = publish("ga:order 1").await?;
    let html = tokio::fs::read_to_string(directory.join("index.html")).await?;
    let document = scraper::Html::parse_document(&html);
    assert_eq!(select(&document, ".site-footer").len(), 1);
    assert_eq!(
        select(&document, "nav.project-navigation").len(),
        1
    );
    assert_eq!(select(&document, ".site-search-open").len(), 1);
    assert!(directory.join("search-index.json").exists());
    Ok(())
}

#[tokio::test]
async fn invalid_footer_and_tokens_fail() -> Result<()> {
    for (profile, extra) in [
        (
            "ga:siteFooter <urn:footer>",
            "<urn:spec> ga:url \"/abs/\" .",
        ),
        (
            "ga:siteFooter <urn:footer>",
            "<urn:note1> dct:description \"![x](y.png)\" .",
        ),
        (
            "ga:siteFooter <urn:footer>",
            "<urn:about> ga:footerLink <urn:omg> .",
        ),
        ("ga:projectNavigation \"no\"", ""),
        (
            "ga:order 1",
            "<urn:theme> ga:mutedColor \"red;}\" .",
        ),
    ] {
        let source = tempfile::tempdir()?;
        let definitions = source.path().join("project.ttl");
        tokio::fs::write(
            &definitions,
            format!(
                "{}\n{extra}\n",
                DEFINITIONS.replace("PROFILE", profile)
            ),
        )
        .await?;
        tokio::fs::write(
            source.path().join("index.md"),
            "---\ntitle: Home\npermalink: /\n---\n# Example\n\nHome.\n",
        )
        .await?;
        let result = async {
            let collected = website::collect(
                source.path(),
                &definitions,
                "urn:project",
                "urn:target",
                2,
            )
            .await?;
            let output = tempfile::tempdir()?;
            Publisher::new(collected.dataset)
                .with_edition(
                    grapharch::publication::pdf_name::Edition::Default,
                )
                .render(&collected.plan, &output.path().join("output"))
                .await?;
            anyhow::Ok(())
        }
        .await;
        assert!(
            result.is_err(),
            "{profile} / {extra} must be rejected"
        );
    }
    Ok(())
}
