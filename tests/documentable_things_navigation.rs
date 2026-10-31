use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        website,
    },
    scraper::{Html, Selector},
    serde::Deserialize,
    std::path::Path,
};

const CATALOGUE_PAGES: [&str; 8] = [
    "documentable-things/README.md",
    "documentable-things/authored-documentation.md",
    "documentable-things/ontologies.md",
    "documentable-things/ontology-families.md",
    "documentable-things/shacl-shapes.md",
    "documentable-things/published-specifications.md",
    "usage/skos-taxonomies.md",
    "usage/use-case-trees.md",
];

#[derive(Deserialize)]
struct Page {
    title:     String,
    parent:    Option<String>,
    permalink: String,
}

async fn metadata(relative: &str) -> Result<(String, Page)> {
    let source =
        tokio::fs::read_to_string(Path::new("docs").join(relative)).await?;
    let (frontmatter, _) = source
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("missing frontmatter: {relative}"))?
        .split_once("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("unclosed frontmatter: {relative}"))?;
    Ok((
        frontmatter.into(),
        serde_yaml::from_str(frontmatter)?,
    ))
}

#[tokio::test]
async fn catalogue_is_top_level_and_type_guides_keep_their_public_urls()
-> Result<()> {
    let (_, root) = metadata(CATALOGUE_PAGES[0]).await?;
    assert_eq!(root.title, "Documentable Things");
    assert!(root.parent.is_none());
    assert_eq!(root.permalink, "/documentable-things/");
    for path in &CATALOGUE_PAGES[1..] {
        let (_, page) = metadata(path).await?;
        assert_eq!(
            page.parent.as_deref(),
            Some("Documentable Things"),
            "{path} belongs in the recognition catalogue"
        );
    }
    for (path, route) in [
        (
            "usage/skos-taxonomies.md",
            "/usage/skos-taxonomies/",
        ),
        (
            "usage/use-case-trees.md",
            "/usage/use-case-trees/",
        ),
    ] {
        assert_eq!(metadata(path).await?.1.permalink, route);
    }
    assert_eq!(
        metadata("usage/ontology-families.md").await?.1.title,
        "Publish an ontology family"
    );
    assert_eq!(
        metadata("usage/ontology-families.md")
            .await?
            .1
            .parent
            .as_deref(),
        Some("Usage"),
        "publishing workflows remain under Usage"
    );
    Ok(())
}

#[tokio::test]
async fn collected_navigation_separates_documentable_types_from_usage_without_live_gallery()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("grapharch.ttl"), r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:catalogue:project> a ga:Project; dct:title "Grapharch";ga:scope <urn:catalogue:scope>;ga:target <urn:catalogue:website> .
<urn:catalogue:scope> a ga:Scope . <urn:catalogue:profile> a ga:Profile .
<urn:catalogue:website> a ga:Target;dct:title "Catalogue navigation";ga:scope <urn:catalogue:scope>;ga:profile <urn:catalogue:profile>;ga:renderer "website";ga:destination "site" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\nnav_exclude: true\n---\n# Home\n",
    )
    .await?;
    // bounded: ten metadata-only pages exercise the real collector/navigation
    // renderer.
    for relative in CATALOGUE_PAGES
        .iter()
        .copied()
        .chain(["usage/README.md", "usage/ontology-families.md"])
    {
        let (frontmatter, page) = metadata(relative).await?;
        let path = source.path().join(relative);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(
            path,
            format!("---\n{frontmatter}\n---\n# {}\n", page.title),
        )
        .await?;
    }
    let built = website::collect(
        source.path(),
        &source.path().join("grapharch.ttl"),
        "urn:catalogue:project",
        "urn:catalogue:website",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(built.dataset)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = Html::parse_document(&html);
    let roots =
        Selector::parse(".site-navigation nav > ul > li > details").unwrap();
    let summary = Selector::parse("summary > a").unwrap();
    let links = Selector::parse("a").unwrap();
    let catalogue = document
        .select(&roots)
        .find(|group| {
            group.select(&summary).next().is_some_and(|a| {
                a.text().collect::<String>() == "Documentable Things"
            })
        })
        .expect("catalogue must be a top-level navigation group");
    assert!(
        catalogue
            .select(&links)
            .any(|a| a.text().collect::<String>() == "SKOS taxonomies")
    );
    assert!(
        catalogue
            .select(&links)
            .any(|a| a.text().collect::<String>() == "EKGF use-case trees")
    );
    let usage = document
        .select(&roots)
        .find(|group| {
            group
                .select(&summary)
                .next()
                .is_some_and(|a| a.text().collect::<String>() == "Usage")
        })
        .expect("Usage remains a separate group");
    assert!(!usage.select(&links).any(|a| {
        matches!(
            a.text().collect::<String>().as_str(),
            "SKOS taxonomies" | "EKGF use-case trees"
        )
    }));
    assert!(usage.select(&links).any(|a| {
        a.value().attr("href") == Some("usage/ontology-families/")
    }));
    Ok(())
}
