//! Readable routes of paginated specification pages: every page except the
//! home page is the `index.html` of a directory named by its page slug.
#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{Publication, PublishOutcome, Publisher},
        showcases,
    },
};

const PAGE_SLUG: &str = "https://ekgf.org/ontology/grapharch#pageSlug";

/// Publish the standards example with the paginated renderer after `edit`.
async fn publish(
    edit: impl FnOnce(String) -> Result<String>,
) -> Result<(tempfile::TempDir, Publication)> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#renderer",
        definitions::iri(
            "https://ekgf.org/ontology/grapharch#SpecificationPagesRenderer",
        )?,
    )?;
    tokio::fs::write(&path, edit(fixture)?).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    source.close()?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok((output, *publication))
}

#[tokio::test]
async fn paginated_pages_are_routed_by_title_or_explicit_slug() -> Result<()> {
    let (_output, publication) = publish(|fixture| {
        definitions::set_property(
            &fixture,
            "urn:specification:changes",
            PAGE_SLUG,
            definitions::literal("changes"),
        )
    })
    .await?;
    let mut pages: Vec<_> = publication
        .manifest
        .files
        .iter()
        .filter(|file| file.ends_with(".html"))
        .map(String::as_str)
        .collect();
    pages.sort_unstable();
    assert_eq!(
        pages,
        [
            "changes/index.html",
            "index.html",
            "publication-requirements/index.html",
            "references/index.html",
        ],
        "an explicit slug names its page; other pages use their titles"
    );
    let home =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let page = tokio::fs::read_to_string(
        publication.directory.join("changes/index.html"),
    )
    .await?;
    for (html, fragment) in [
        (&page, "href=\"../style.css\""),
        (&page, "href=\"../index.html\""),
        (
            &page,
            "data-search-index=\"../search-index.json\"",
        ),
        (&home, "href=\"style.css\""),
    ] {
        assert!(html.contains(fragment), "missing {fragment}");
    }
    let index: Vec<serde_json::Value> = serde_json::from_slice(
        &tokio::fs::read(publication.directory.join("search-index.json"))
            .await?,
    )?;
    assert!(
        index.iter().any(|entry| {
            entry["href"]
                .as_str()
                .is_some_and(|href| href.starts_with("changes/index.html#"))
        }),
        "search entries are relative to the publication root"
    );
    Ok(())
}

#[tokio::test]
async fn paginated_page_slug_collisions_are_errors() -> Result<()> {
    let error = publish(|fixture| {
        definitions::set_property(
            &fixture,
            "urn:specification:model",
            PAGE_SLUG,
            definitions::literal("references"),
        )
    })
    .await
    .err()
    .expect("two pages cannot share one route");
    let message = format!("{error:#}");
    assert!(
        message.contains("references/") &&
            message.contains("Publication requirements") &&
            message.contains("References"),
        "{message}"
    );
    Ok(())
}

#[tokio::test]
async fn invalid_explicit_page_slugs_are_errors() -> Result<()> {
    let error = publish(|fixture| {
        definitions::set_property(
            &fixture,
            "urn:specification:changes",
            PAGE_SLUG,
            definitions::literal("Change History"),
        )
    })
    .await
    .err()
    .expect("an explicit slug is never normalized");
    let message = format!("{error:#}");
    assert!(
        message.contains("Change history") &&
            message.contains("\"Change History\""),
        "{message}"
    );
    Ok(())
}
