use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn project_pages_render_source_owned_sections_without_documentation_chrome()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Example project"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:style "ekgf-project" .
<urn:target> a ga:Target; dct:title "Project website"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
"#).await?;
    tokio::fs::write(
        input.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:home\n---\n# Example project\n\nAn introduction.\n\n[Get \
         started](guide.md)\n\n## Benefits\n\n### First benefit\n\nA \
         source-owned explanation.\n\n### Second benefit\n\nAnother \
         explanation.\n\n## Next steps\n\n[Read the guide](guide.md)\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("guide.md"),
        "---\ntitle: Guide\npermalink: /guide/\ngrapharch:\n  subject: \
         urn:guide\n---\n[Home](index.md)\n",
    )
    .await?;
    let collected = website::collect(
        input.path(),
        &settings,
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
    input.close()?;
    let restored = Dataset::memory()?;
    restored.import(export.path().join("kg.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    let count = |selector| {
        doc.select(&scraper::Selector::parse(selector).unwrap())
            .count()
    };
    assert_eq!(count(".project-hero"), 1);
    assert_eq!(count(".project-identity-row"), 0);
    assert_eq!(count(".project-section"), 2);
    assert_eq!(count(".project-card"), 2);
    assert_eq!(count(".project-hero .project-actions"), 1);
    assert_eq!(
        count(".project-actions a[href=\"https://example.org/intro\"]"),
        0
    );
    assert_eq!(
        count(".site-navigation, .page-outline, .breadcrumbs, .page-neighbors"),
        0
    );
    assert!(html.contains("href=\"guide/\""));
    let nested = tokio::fs::read_to_string(
        publication.directory.join("guide/index.html"),
    )
    .await?;
    assert!(nested.contains("href=\"../\""));
    let guide = scraper::Html::parse_document(&nested);
    assert_eq!(
        guide
            .select(&scraper::Selector::parse(".project-hero h1").unwrap())
            .count(),
        1
    );
    Ok(())
}
