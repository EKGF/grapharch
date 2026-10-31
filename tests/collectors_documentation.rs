use {
    anyhow::Result,
    pulldown_cmark::{Event, Parser, Tag},
    serde::Deserialize,
    std::{collections::HashSet, path::Path},
};

const PAGES: &[&str] = &[
    "README.md",
    "rdf-files.md",
    "rdf-urls.md",
    "markdown-files.md",
    "sparql-files.md",
    "sparql-endpoints.md",
    "published-specifications.md",
    "legal-preambles.md",
    "repositories.md",
    "assets-and-code.md",
];

#[derive(Deserialize)]
struct Metadata {
    title:     String,
    parent:    Option<String>,
    permalink: String,
}

#[tokio::test]
async fn collectors_catalogue_has_unique_navigation_practical_examples_and_resolved_links()
-> Result<()> {
    let mut titles = HashSet::new();
    let mut routes = HashSet::new();
    for (index, name) in PAGES.iter().enumerate() {
        let path = Path::new("docs/collectors").join(name);
        let source = tokio::fs::read_to_string(&path).await?;
        let (frontmatter, body) = source
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap();
        let metadata: Metadata = serde_yaml::from_str(frontmatter)?;
        assert!(
            titles.insert(metadata.title.clone()),
            "duplicate navigation title"
        );
        assert!(
            routes.insert(metadata.permalink.clone()),
            "duplicate permalink"
        );
        if index == 0 {
            assert_eq!(metadata.title, "Collectors");
            assert!(metadata.parent.is_none());
            assert_eq!(metadata.permalink, "/collectors/");
        } else {
            assert_eq!(metadata.parent.as_deref(), Some("Collectors"));
            assert!(
                body.contains("```"),
                "{name} needs a practical example"
            );
            assert!(
                body.contains("documentable-things/") ||
                    body.contains("usage/skos-taxonomies") ||
                    body.contains("usage/use-case-trees"),
                "{name} needs an explicit documentable-thing mapping"
            );
        }
        for event in Parser::new(body) {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                let destination = dest_url.split('#').next().unwrap();
                if destination.is_empty() ||
                    url::Url::parse(destination).is_ok()
                {
                    continue;
                }
                assert!(
                    tokio::fs::try_exists(
                        path.parent().unwrap().join(destination)
                    )
                    .await?,
                    "{name}: broken local link {destination}"
                );
            }
        }
    }
    let sources =
        tokio::fs::read_to_string("docs/features/sources/README.md").await?;
    assert!(sources.contains("../../collectors/README.md"));
    Ok(())
}

#[tokio::test]
async fn collectors_are_a_top_level_generated_navigation_group() -> Result<()> {
    use {
        grapharch::publication::{
            render::{PublishOutcome, Publisher},
            website,
        },
        scraper::{Html, Selector},
    };
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("grapharch.ttl"), r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:collectors:project> a ga:Project; dct:title "Grapharch";ga:scope <urn:collectors:scope>;ga:target <urn:collectors:website> .
<urn:collectors:scope> a ga:Scope . <urn:collectors:profile> a ga:Profile .
<urn:collectors:website> a ga:Target;dct:title "Collectors";ga:scope <urn:collectors:scope>;ga:profile <urn:collectors:profile>;ga:renderer "website";ga:destination "site" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\nnav_exclude: true\n---\n# Home\n",
    )
    .await?;
    // bounded: ten metadata-only pages exercise actual collection/navigation
    // without the live showcase gallery or external source repositories.
    for name in PAGES {
        let original =
            tokio::fs::read_to_string(Path::new("docs/collectors").join(name))
                .await?;
        let (frontmatter, _) = original
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap();
        let metadata: Metadata = serde_yaml::from_str(frontmatter)?;
        let path = source.path().join("collectors").join(name);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(
            path,
            format!("---\n{frontmatter}\n---\n# {}\n", metadata.title),
        )
        .await?;
    }
    let built = website::collect(
        source.path(),
        &source.path().join("grapharch.ttl"),
        "urn:collectors:project",
        "urn:collectors:website",
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
    let summary = Selector::parse("summary > a").unwrap();
    let catalogue = document
        .select(
            &Selector::parse(".site-navigation nav > ul > li > details")
                .unwrap(),
        )
        .find(|group| {
            group.select(&summary).next().is_some_and(|link| {
                link.text().collect::<String>() == "Collectors"
            })
        })
        .expect("Collectors needs a top-level navigation group");
    let links = Selector::parse("li a").unwrap();
    assert_eq!(catalogue.select(&links).count(), PAGES.len());
    Ok(())
}
