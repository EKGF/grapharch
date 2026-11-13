use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn directory_navigation_is_collected_from_source_owned_pages_files()
-> Result<()> {
    check_navigation(true).await
}

#[tokio::test]
async fn indexless_sections_and_wildcards_before_explicit_paths() -> Result<()>
{
    check_navigation(false).await
}

async fn check_navigation(with_index: bool) -> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("concepts")).await?;
    for (path, text) in [
        ("index.md", "---\ntitle: Home\n---\n# Home\n"),
        (
            "concepts/index.md",
            "---\ntitle: Concepts\n---\n# Concepts\n",
        ),
        ("concepts/a.md", "# Topic A\n"),
        (
            "concepts/b.md",
            "---\ntitle: Topic B\n---\n# Topic B\n",
        ),
    ] {
        if with_index || path != "concepts/index.md" {
            tokio::fs::write(source.path().join(path), text).await?;
        }
    }
    tokio::fs::write(
        source.path().join(".pages.yaml"),
        "nav:\n- Introduction: index.md\n- Learn: concepts\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("concepts/.pages.yaml"),
        if with_index {
            "nav:\n- index.md\n- B first: b.md\n- a.md\n"
        } else {
            "nav:\n- ...\n- B last: b.md\n"
        },
    )
    .await?;
    let defs = source.path().join("project.ttl");
    tokio::fs::write(&defs, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "A documentation site"; ga:target <urn:target>; ga:collector <urn:pages> .
<urn:pages> a ga:CollectorUse; ga:collectorType ga:DocumentationNavigationCollector; ga:source "."; ga:navigationProfile <urn:profile> .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:layout "documentation" .
"#).await?;
    let built = website::collect(
        source.path(),
        &defs,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let dataset = grapharch::publication::dataset::Dataset::memory()?;
    dataset.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("published")
    };
    let html = tokio::fs::read_to_string(
        publication.directory.join("concepts/a/index.html"),
    )
    .await?;
    let doc = scraper::Html::parse_document(&html);
    let primary = doc
        .select(&scraper::Selector::parse(".primary-navigation a").unwrap())
        .map(|e| e.text().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(primary, ["Introduction", "Learn"]);
    assert!(
        doc.select(
            &scraper::Selector::parse(".primary-navigation details").unwrap()
        )
        .next()
        .is_none()
    );
    let selector = scraper::Selector::parse(".sidebar-navigation a").unwrap();
    let labels = doc
        .select(&selector)
        .map(|e| e.text().collect::<String>())
        .collect::<Vec<_>>();
    let adjacent = doc
        .select(&scraper::Selector::parse(".page-neighbors a").unwrap())
        .map(|e| e.text().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(
        adjacent,
        if with_index {
            vec!["PreviousB first"]
        } else {
            vec!["PreviousIntroduction", "NextB last"]
        }
    );
    if with_index {
        assert_eq!(labels, [
            "Introduction",
            "Learn",
            "B first",
            "Topic A"
        ]);
    } else {
        assert_eq!(labels, ["Introduction", "Topic A", "B last"]);
        assert!(html.contains("Learn"));
    }
    Ok(())
}

#[tokio::test]
async fn excluded_generated_sources_do_not_enter_navigation_discovery()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("docs/generated")).await?;
    tokio::fs::write(source.path().join("docs/index.md"), "# Home").await?;
    tokio::fs::write(
        source.path().join("docs/.pages.yaml"),
        "nav:\n- index.md\n- generated\n- ...\n",
    )
    .await?;
    for index in 0..513 {
        tokio::fs::write(
            source.path().join(format!("docs/generated/{index}.md")),
            "# Excluded generated page",
        )
        .await?;
    }
    tokio::fs::create_dir_all(source.path().join("docs/assets/nested")).await?;
    tokio::fs::write(
        source.path().join("docs/assets/nested/icon.txt"),
        "not documentation",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:p> a ga:Project; ga:contentSource "docs"; ga:excludeSourcePath "generated"; ga:collector <urn:n>; ga:target <urn:t>.
<urn:n> a ga:CollectorUse; ga:collectorType ga:DocumentationNavigationCollector; ga:source "docs"; ga:navigationProfile <urn:f>.
<urn:t> a ga:Target; ga:scope <urn:s>; ga:profile <urn:f>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site".
<urn:s> a ga:Scope. <urn:f> a ga:Profile; ga:layout "documentation"."#).await?;
    let built =
        website::collect(source.path(), &config, "urn:p", "urn:t", 2).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(site) = Publisher::new(built.dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication failed")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(!html.contains("Excluded generated page"));
    Ok(())
}
