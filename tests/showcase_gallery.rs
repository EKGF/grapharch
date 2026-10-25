#[path = "support/family_repository.rs"]
mod family_repository;

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
async fn gallery_preserves_live_showcase_artifacts_in_the_exported_graph()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(
        &repo.join("examples"),
        &source.path().join("examples"),
    )?;
    // The acceptance test explicitly selects a local bare repository, never the
    // network.
    let family = source.path().join("examples/ekg-ontologies");
    let repository = tempfile::tempdir()?;
    let (url, commit) = family_repository::bare_copy(
        &family.join("sources"),
        repository.path(),
    )?;
    let mut config: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(family.join("showcase.yaml"))?,
    )?;
    config["source"]["url"] = url.into();
    config["source"]["ref"] = commit.into();
    std::fs::write(
        family.join("showcase.yaml"),
        serde_yaml::to_string(&config)?,
    )?;
    let docs = source.path().join("docs");
    std::fs::create_dir(&docs)?;
    std::fs::write(
        docs.join("grapharch.ttl"),
        include_str!("../docs/grapharch.ttl").replace(
            "    ga:ontologySource \"other/ontology/publication.ttl\" ;\n",
            "",
        ),
    )?;
    std::fs::write(
        docs.join("index.md"),
        "---\ntitle: Gallery\ngrapharch:\n  subject: urn:gallery:page\n  \
         showcase_gallery: true\n---\n# Showcase gallery\n",
    )?;
    let built = website::collect(
        &docs,
        &docs.join("grapharch.ttl"),
        "urn:grapharch:docs",
        "urn:grapharch:docs:website",
        4,
    )
    .await?;
    let mut placeholders = built
        .dataset
        .select(
            "PREFIX ga: <https://ekgf.org/ontology/grapharch#> SELECT ?asset \
             WHERE { GRAPH ?g { ?asset a ga:BinaryAsset, ga:Fragment } } \
             LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        placeholders.next().await?.is_none(),
        "binary artifacts must be explicit assets, not empty documentation \
         fragments"
    );
    let mut assets = built
        .dataset
        .select(format!(
            "PREFIX ga: <https://ekgf.org/ontology/grapharch#> SELECT ?asset \
             WHERE {{ GRAPH ?g {{ {} ga:asset ?asset }} }} LIMIT 1",
            built.plan.id
        ))
        .await?;
    assert!(
        assets.next().await?.is_some(),
        "the publication plan must explicitly select its assets"
    );
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    std::fs::remove_dir_all(repository.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("expected gallery publication")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    let document = scraper::Html::parse_document(&html);
    let cards = scraper::Selector::parse(".showcase-card").unwrap();
    assert_eq!(document.select(&cards).count(), 6);
    assert_eq!(
        document
            .select(
                &scraper::Selector::parse("iframe[sandbox][loading=lazy]")
                    .unwrap()
            )
            .count(),
        6
    );
    for link in document.select(
        &scraper::Selector::parse(".showcase-card a[data-artifact]").unwrap(),
    ) {
        let path = publication
            .directory
            .join(link.value().attr("href").unwrap());
        assert!(
            path.is_file(),
            "missing artifact {}",
            path.display()
        );
        if path.extension().is_some_and(|ext| ext == "pdf") {
            assert!(std::fs::read(path)?.starts_with(b"%PDF-"));
        }
    }
    assert!(html.contains("artifacts/dprod/"));
    assert!(html.contains("DPROD: native ontology and SHACL"));
    assert!(!html.contains("artifacts/rdf11/"));
    assert!(!html.contains("W3C RDF 1.1 N-Triples"));
    assert!(html.contains("expected source diagnostic"));
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
            )?;
        } else {
            std::fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}
