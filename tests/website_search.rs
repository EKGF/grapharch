use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    serde::Deserialize,
};
#[derive(Deserialize)]
struct Page {
    title: String,
    route: String,
    text:  String,
}
#[tokio::test]
async fn documentation_search_uses_retained_page_content_after_source_removal()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Knowledge guide"; ga:target <urn:site> .
<urn:site> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:style "ekgf"; ga:layout "documentation" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "# Introduction\n\nWelcome.",
    )
    .await?;
    tokio::fs::write(
        source.path().join("guide.md"),
        "# Governance\n\n## Decisions\n\nAn unusual **cormorant** example \
         with `RDF`.",
    )
    .await?;
    let built = website::collect(
        source.path(),
        &definitions,
        "urn:project",
        "urn:site",
        2,
    )
    .await?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let pages: Vec<Page> = serde_json::from_slice(
        &tokio::fs::read(result.directory.join("search-index.json")).await?,
    )?;
    assert_eq!(pages.len(), 2);
    let guide = pages
        .iter()
        .find(|p| p.route == "guide/")
        .expect("guide indexed");
    assert_eq!(guide.title, "Governance");
    assert!(guide.text.contains("cormorant"));
    assert!(!guide.text.contains("**"));
    let html =
        tokio::fs::read_to_string(result.directory.join("guide/index.html"))
            .await?;
    assert!(html.contains("aria-label=\"Search documentation\""));
    assert!(html.contains("data-search-index=\"../search-index.json\""));
    assert!(html.contains("search.js"));
    assert!(result.directory.join("search.js").exists());
    Ok(())
}
