//! Repository links lead to the registered websites published from them,
//! but only where those websites have a page; everything else stays on the
//! code host.
use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

const DEFINITIONS: &str = r#"@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Records"; ga:target <urn:target>; ga:repository <https://github.com/example-org/records> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer ga:WebsiteRenderer; ga:destination "site" .
<urn:profile> a ga:Profile REGISTRY .
<urn:records-site> a ga:RegisteredSite; ga:repository <https://github.com/example-org/records>; ga:url "https://example-org.github.io/records/" .
<urn:models-site> a ga:RegisteredSite; ga:repository <https://github.com/example-org/models>; ga:url "https://example-org.github.io/models/"; ga:contentSource "docs"; ga:gitRef "main" .
"#;

const PAGE: &str = "---\ntitle: Home\npermalink: /\n---\n# Records\n\n\
- [models](https://github.com/example-org/models)\n\
- [guide](https://github.com/example-org/models/blob/main/docs/guide/intro.md#start)\n\
- [models index](https://github.com/example-org/models/blob/main/docs/index.md)\n\
- [old guide](https://github.com/example-org/models/blob/v1/docs/guide/intro.md)\n\
- [code](https://github.com/example-org/models/blob/main/src/lib.rs)\n\
- [outside docs](https://github.com/example-org/models/blob/main/README.md)\n\
- [tree](https://github.com/example-org/models/tree/main/docs)\n\
- [issues](https://github.com/example-org/models/issues)\n\
- [own page](https://github.com/example-org/records/blob/main/decisions.md#why)\n\
- [own missing](https://github.com/example-org/records/blob/main/missing.md)\n\
- [unregistered](https://github.com/example-org/other/blob/main/docs/a.md)\n";

async fn links(registry: &str) -> Result<Vec<(String, String)>> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(&path, DEFINITIONS.replace("REGISTRY", registry)).await?;
    tokio::fs::write(source.path().join("index.md"), PAGE).await?;
    tokio::fs::write(
        source.path().join("decisions.md"),
        "---\ntitle: Decisions\n---\n# Decisions\n\n## Why\n\nReasons.\n",
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
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    Ok(document
        .select(&scraper::Selector::parse("article li a").unwrap())
        .map(|link| {
            (
                link.text().collect(),
                link.value().attr("href").unwrap().to_owned(),
            )
        })
        .collect())
}

#[tokio::test]
async fn registered_repositories_link_to_their_website_pages() -> Result<()> {
    let links =
        links("; ga:siteRegistry ( <urn:records-site> <urn:models-site> )")
            .await?;
    let expected = [
        ("models", "https://example-org.github.io/models/"),
        (
            "guide",
            "https://example-org.github.io/models/guide/intro/#start",
        ),
        (
            "models index",
            "https://example-org.github.io/models/",
        ),
        (
            "old guide",
            "https://github.com/example-org/models/blob/v1/docs/guide/intro.md",
        ),
        (
            "code",
            "https://github.com/example-org/models/blob/main/src/lib.rs",
        ),
        (
            "outside docs",
            "https://github.com/example-org/models/blob/main/README.md",
        ),
        (
            "tree",
            "https://github.com/example-org/models/tree/main/docs",
        ),
        (
            "issues",
            "https://github.com/example-org/models/issues",
        ),
        ("own page", "decisions/#why"),
        (
            "own missing",
            "https://github.com/example-org/records/blob/main/missing.md",
        ),
        (
            "unregistered",
            "https://github.com/example-org/other/blob/main/docs/a.md",
        ),
    ];
    assert_eq!(
        links,
        expected
            .iter()
            .map(|(text, href)| ((*text).to_owned(), (*href).to_owned()))
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn without_a_registry_repository_links_are_unchanged() -> Result<()> {
    for (_, href) in links("").await? {
        assert!(href.starts_with("https://github.com/"), "{href}");
    }
    Ok(())
}

#[tokio::test]
async fn invalid_registries_fail() -> Result<()> {
    for registry in [
        "; ga:siteRegistry ( <urn:records-site> <urn:records-site> )",
        "; ga:siteRegistry ( <urn:unknown> )",
        "; ga:siteRegistry ( <urn:bad> ) . <urn:bad> a ga:RegisteredSite; \
         ga:repository <https://github.com/example-org/models>; ga:url \
         \"https://example-org.github.io/models\"",
    ] {
        assert!(links(registry).await.is_err(), "{registry}");
    }
    Ok(())
}
