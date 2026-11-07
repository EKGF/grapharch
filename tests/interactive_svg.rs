//! Explicit SVG navigation is retained separately from its HTML presentation.
use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    scraper::{Html, Selector},
};
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 520"><style>.level-row{fill:#fefce8}@media(prefers-color-scheme:dark){.level-row{fill:#202027}} .site-header{display:none}</style><a href="/topic/#topic"><rect class="level-row" width="1000" height="520"/><text x="50" y="100">Open topic</text></a></svg>"##;

#[tokio::test]
async fn interactive_svg_keeps_original_bytes_and_projects_validated_links_after_restore()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:interactive:project> a ga:Project; ga:target <urn:interactive:site> .
<urn:interactive:scope> a ga:Scope . <urn:interactive:profile> a ga:Profile .
<urn:interactive:site> a ga:Target; ga:scope <urn:interactive:scope>; ga:profile <urn:interactive:profile>; ga:renderer "website"; ga:destination "site" ."#).await?;
    tokio::fs::write(source.path().join("diagram.svg"), SVG).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Navigation\npermalink: /\n---\n# Navigation\n\n![Static \
         diagram](diagram.svg \"Ordinary author title\")\n\n![Interactive \
         diagram](diagram.svg \"grapharch:interactive-svg\")\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("topic.md"),
        "---\ntitle: Topic\npermalink: /topic/\n---\n# Topic\nThe target \
         remains source-owned.\n",
    )
    .await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:interactive:project",
        "urn:interactive:site",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    collected.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = Html::parse_document(&html);
    let object = document
        .select(&Selector::parse("object.interactive-svg").unwrap())
        .next()
        .expect("explicit interactive presentation");
    assert_eq!(object.value().attr("type"), Some("image/svg+xml"));
    assert_eq!(
        object.value().attr("aria-label"),
        Some("Interactive diagram")
    );
    let projected = tokio::fs::read_to_string(
        publication
            .directory
            .join(object.value().attr("data").unwrap()),
    )
    .await?;
    assert!(projected.contains("../topic/#topic"));
    assert!(projected.contains("target=\"_top\""));
    let image = document
        .select(&Selector::parse("img[alt='Static diagram']").unwrap())
        .next()
        .expect("ordinary image stays an image");
    assert_eq!(
        image.value().attr("title"),
        Some("Ordinary author title")
    );
    assert_eq!(
        tokio::fs::read_to_string(
            publication
                .directory
                .join(image.value().attr("src").unwrap())
        )
        .await?,
        SVG
    );
    assert!(!html.contains("title=\"grapharch:interactive-svg\""));
    Ok(())
}

#[tokio::test]
async fn unresolved_svg_navigation_prevents_partial_publication() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:interactive:project> a ga:Project; ga:target <urn:interactive:site> .
<urn:interactive:scope> a ga:Scope . <urn:interactive:profile> a ga:Profile .
<urn:interactive:site> a ga:Target; ga:scope <urn:interactive:scope>; ga:profile <urn:interactive:profile>; ga:renderer "website"; ga:destination "site" ."#).await?;
    tokio::fs::write(source.path().join("diagram.svg"), SVG).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Navigation\npermalink: /\n---\n# \
         Navigation\n\n![Navigation](diagram.svg \
         \"grapharch:interactive-svg\")\n",
    )
    .await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:interactive:project",
        "urn:interactive:site",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let error = match Publisher::new(collected.dataset)
        .render(&collected.plan, output.path())
        .await
    {
        Err(error) => error,
        Ok(_) => panic!("unknown SVG navigation must fail publication"),
    };
    assert!(format!("{error:#}").contains("unresolved documentation link"));
    assert!(
        !tokio::fs::try_exists(output.path().join("site/index.html")).await?
    );
    Ok(())
}
