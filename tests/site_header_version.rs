use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn a_project_without_a_version_shows_no_chip_and_credits_the_generator()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let settings = source.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:header:project> a ga:Project;dct:title "Grapharch";ga:repository <https://github.com/EKGF/grapharch>;ga:scope <urn:header:scope>;ga:target <urn:header:website> .
<urn:header:scope> a ga:Scope . <urn:header:profile> a ga:Profile .
<urn:header:website> a ga:Target;dct:title "Documentation";ga:scope <urn:header:scope>;ga:profile <urn:header:profile>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:header:home\n---\n\n# Home\n\nThe generator version belongs in \
         the credit, not the header.\n",
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
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    // The generator's build is not the documented project's version.
    assert!(
        document
            .select(&scraper::Selector::parse(".brand-version").unwrap())
            .next()
            .is_none(),
        "the header must not show the generator's version as the project's"
    );
    assert!(
        !document
            .select(&scraper::Selector::parse("header").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>()
            .contains(env!("GRAPHARCH_BUILD_LABEL"))
    );
    let binary = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("--version")
        .output()
        .await?;
    assert!(binary.status.success());
    assert_eq!(
        String::from_utf8(binary.stdout)?.trim(),
        format!(
            "grapharch {}\n{}",
            env!("GRAPHARCH_BUILD_LABEL"),
            env!("GRAPHARCH_BUILD_DETAILS")
        )
    );
    let canonical =
        tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
            .arg("-V")
            .output()
            .await?;
    assert!(canonical.status.success());
    assert_eq!(
        String::from_utf8(canonical.stdout)?.trim(),
        format!("grapharch {}", env!("CARGO_PKG_VERSION"))
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
            "GraphArch {}",
            env!("GRAPHARCH_BUILD_LABEL")
        ))
    );
    let producer = footer
        .select(&scraper::Selector::parse(".generator-build").unwrap())
        .next()
        .expect("structured producer metadata");
    assert_eq!(
        producer.value().attr("data-build-attempt"),
        Some(env!("GRAPHARCH_BUILD_ATTEMPT"))
    );
    assert_eq!(
        producer.value().attr("data-build-run"),
        Some(env!("GRAPHARCH_BUILD_RUN_URL"))
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
