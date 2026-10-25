#[path = "support/definitions.rs"]
mod definitions_fixture;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::Path,
};

#[tokio::test]
async fn ontology_spec_is_published_with_the_companion_site_from_a_restored_graph()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ontology = source.path().join("other/ontology");
    std::fs::create_dir_all(&ontology)?;
    std::fs::copy(
        repo.join("docs/other/ontology/publication.ttl"),
        ontology.join("publication.ttl"),
    )?;
    std::fs::write(
        source.path().join("grapharch.ttl"),
        definitions_fixture::edit_definitions(
            &std::fs::read_to_string(repo.join("docs/grapharch.ttl"))?,
            &["showcasesSource"],
            None,
        )? + "\n<urn:unrelated:ontology> a <http://www.w3.org/2002/07/owl#Ontology> .\n",
    )?;
    std::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:spec-test:home\n---\n# GraphArch\n[Ontology \
         specification](spec/develop/index.html)\n",
    )?;
    let built = website::collect(
        source.path(),
        &source.path().join("grapharch.ttl"),
        "urn:grapharch:docs",
        "urn:grapharch:docs:website",
        4,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("expected publication")
    };
    let spec = publication.directory.join("spec/develop");
    let html = std::fs::read_to_string(spec.join("index.html"))?;
    let rendered_text = scraper::Html::parse_document(&html)
        .root_element()
        .text()
        .collect::<String>();
    for text in [
        "GraphArch Documentation and Publication Ontology",
        "Editor's Draft",
        "ga:Fragment",
        "ga:ontologySource",
        "ga:original",
        "union of",
        "collector",
        "DEFAULT",
    ] {
        assert!(rendered_text.contains(text), "missing {text}");
    }
    assert!(
        !html.contains("urn:grapharch:skolem:"),
        "opaque blank-node identifiers are not documentation"
    );
    assert!(html.contains("class=\"pdf-download\""));
    assert!(std::fs::read(spec.join("document.pdf"))?.starts_with(b"%PDF-"));
    assert!(std::fs::read(spec.join("documentation.nq"))?.len() > 1024);
    let page = scraper::Html::parse_document(&html);
    let links = scraper::Selector::parse("a[href^='#']").unwrap();
    for link in page.select(&links) {
        let anchor = link.value().attr("href").unwrap().trim_start_matches('#');
        assert!(
            page.select(&scraper::Selector::parse("[id]").unwrap())
                .any(|node| node.value().id() == Some(anchor)),
            "broken anchor {anchor}"
        );
    }
    Ok(())
}
