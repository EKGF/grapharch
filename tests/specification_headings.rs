#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
};

async fn specification(renderer: &str) -> Result<String> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#renderer",
        definitions::literal(renderer),
    )?;
    tokio::fs::write(&path, format!("{fixture}\nex:subject <http://www.w3.org/2000/01/rdf-schema#label> \"First label\"@en, \"Other label\"@en .\n")).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    source.close()?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok(
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?,
    )
}

fn assert_structural_links(html: &str) {
    let document = scraper::Html::parse_document(html);
    let sections = scraper::Selector::parse("main section[id]").unwrap();
    let links = scraper::Selector::parse("a.self-link").unwrap();
    let mut checked_quality = false;
    let mut checked_ordinary = false;
    for section in document.select(&sections) {
        let heading = section
            .children()
            .filter_map(scraper::ElementRef::wrap)
            .find(|child| {
                matches!(
                    child.value().name(),
                    "h2" | "h3" | "h4" | "h5" | "h6"
                )
            });
        let Some(heading) = heading else { continue };
        let id = section.value().id().unwrap();
        let link = heading.select(&links).next().unwrap_or_else(|| {
            panic!("structural heading {id} must use the shared permalink")
        });
        assert_eq!(link.text().collect::<String>(), "§");
        assert_eq!(
            link.value().attr("href"),
            Some(format!("#{id}").as_str())
        );
        assert_eq!(
            link.value().attr("aria-label"),
            Some("Link to this section")
        );
        checked_quality |= id == "quality-issues";
        checked_ordinary |= id != "quality-issues";
    }
    assert!(
        checked_quality && checked_ordinary,
        "both generated and authored structural headings must be checked"
    );
}

#[tokio::test]
async fn flat_generated_and_authored_headings_share_permalink_behavior()
-> Result<()> {
    assert_structural_links(&specification("specification").await?);
    Ok(())
}

#[tokio::test]
async fn paginated_generated_and_authored_headings_share_permalink_behavior()
-> Result<()> {
    assert_structural_links(&specification("specification-pages").await?);
    Ok(())
}
