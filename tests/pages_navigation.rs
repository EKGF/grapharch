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
<urn:pages> a ga:Collector; ga:kind "documentation-navigation"; ga:source "."; ga:navigationProfile <urn:profile> .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
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
