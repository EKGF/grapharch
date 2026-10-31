use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn documentation_header_identifies_the_generator_version_from_restored_kg()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let settings = source.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:header:project> a ga:Project;dct:title "Grapharch";ga:repository <https://github.com/EKGF/grapharch>;ga:scope <urn:header:scope>;ga:target <urn:header:website> .
<urn:header:scope> a ga:Scope . <urn:header:profile> a ga:Profile .
<urn:header:website> a ga:Target;dct:title "Documentation";ga:scope <urn:header:scope>;ga:profile <urn:header:profile>;ga:renderer "website";ga:destination "site" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:header:home\n---\n\n# Home\n\nThe generator version belongs in \
         the documentation header.\n",
    )
    .await?;
    let built = website::collect(
        source.path(),
        &settings,
        "urn:header:project",
        "urn:header:website",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let version = document
        .select(
            &scraper::Selector::parse(".site-header .brand-version").unwrap(),
        )
        .next()
        .expect("visible version badge in the header");
    assert_eq!(
        version.text().collect::<String>(),
        format!("Version {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        version
            .select(&scraper::Selector::parse(".visually-hidden").unwrap())
            .next()
            .expect("accessible version label")
            .text()
            .collect::<String>(),
        "Version "
    );
    let context = document
        .select(
            &scraper::Selector::parse(".site-header .brand-context").unwrap(),
        )
        .next()
        .expect("documentation context");
    assert_eq!(
        context.text().collect::<String>(),
        "Documentation"
    );
    let footer = document
        .select(&scraper::Selector::parse("footer").unwrap())
        .next()
        .unwrap();
    assert!(
        footer.text().collect::<String>().contains(&format!(
            "Grapharch {}",
            env!("CARGO_PKG_VERSION")
        ))
    );
    if let Ok(destination) = std::env::var("GRAPHARCH_HEADER_REVIEW") {
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "style.css"] {
            tokio::fs::copy(
                publication.directory.join(name),
                std::path::Path::new(&destination).join(name),
            )
            .await?;
        }
    }
    Ok(())
}
