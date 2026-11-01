#[path = "support/definitions.rs"]
mod definitions;

#[path = "support/family_repository.rs"]
mod family_repository;

#[path = "support/dprod_repository.rs"]
mod dprod_repository;

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
    // This old acceptance corpus predates upstream-owned publication hints.
    // Its explicit test override is distinct from the transport-only live
    // configuration.
    config["definitions"] = "project.ttl".into();
    config["project"] = "urn:ekgf-family:project".into();
    config["target"] = "urn:ekgf-family:specification".into();
    config["source"]["url"] = url.into();
    config["source"]["ref"] = commit.into();
    std::fs::write(
        family.join("showcase.yaml"),
        serde_yaml::to_string(&config)?,
    )?;
    // The published Commons configuration remains live HTTP. This transport-
    // independent gallery acceptance uses a tiny synthetic repository; the
    // dedicated Commons test covers HTTP locally, and Docs CI verifies OMG.
    let commons = source.path().join("examples/omg-commons");
    let commons_sources = tempfile::tempdir()?;
    let corpus = String::from(
        r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct: <http://purl.org/dc/terms/> .
<https://www.omg.org/spec/Commons/Classifiers/Fixture> a owl:Class ; rdfs:label "Synthetic fixture"@en ; dct:description "An offline test resource."@en ; rdfs:isDefinedBy <https://www.omg.org/spec/Commons/Classifiers/> .
"#,
    );
    // bounded: the fixed authored showcase registry has 24 ontology members.
    let mut members = std::collections::BTreeSet::new();
    let definitions = std::fs::read(commons.join("project.ttl"))?;
    for quad in oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(&definitions)
    {
        let quad = quad?;
        if quad.predicate.as_str() ==
            "https://ekgf.org/ontology/artifact-governance#inFamily" &&
            quad.object.to_string() == "<urn:omg-commons:family>"
        {
            members.insert(quad.subject.to_string());
        }
    }
    assert!(!members.is_empty());
    for (index, member) in members.iter().enumerate() {
        let mut body = if member ==
            "<https://www.omg.org/spec/Commons/Classifiers/>"
        {
            corpus.clone()
        } else {
            String::from("@prefix owl: <http://www.w3.org/2002/07/owl#> .\n@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n@prefix dct: <http://purl.org/dc/terms/> .\n")
        };
        body.push_str(&format!(
            "{member} a owl:Ontology ; rdfs:label \"Synthetic offline \
             ontology {index}\"@en ; dct:description \"Gallery fixture, not \
             copied upstream content.\"@en .\n"
        ));
        std::fs::write(
            commons_sources.path().join(format!("ontology-{index}.ttl")),
            body,
        )?;
    }
    let commons_repository = tempfile::tempdir()?;
    let (url, commit) = family_repository::bare_copy(
        commons_sources.path(),
        commons_repository.path(),
    )?;
    let mut config: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(commons.join("showcase.yaml"))?,
    )?;
    let transport = config["source"].as_mapping_mut().expect("source mapping");
    transport.clear();
    transport.insert("kind".into(), "git".into());
    transport.insert("url".into(), url.into());
    transport.insert("ref".into(), commit.into());
    std::fs::write(
        commons.join("showcase.yaml"),
        serde_yaml::to_string(&config)?,
    )?;
    let dprod_repository = tempfile::tempdir()?;
    dprod_repository::configure(
        &source.path().join("examples/dprod"),
        dprod_repository.path(),
    )?;
    let docs = source.path().join("docs");
    std::fs::create_dir(&docs)?;
    std::fs::write(
        docs.join("grapharch.ttl"),
        definitions::edit_definitions(
            include_str!("../docs/grapharch.ttl"),
            &["ontologySource", "collector", "include"],
            None,
        )?,
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
    std::fs::remove_dir_all(commons_repository.path())?;
    std::fs::remove_dir_all(dprod_repository.path())?;
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
    assert_eq!(document.select(&cards).count(), 10);
    assert_eq!(
        document
            .select(
                &scraper::Selector::parse("iframe[sandbox][loading=lazy]")
                    .unwrap()
            )
            .count(),
        10
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
    for card in document.select(&cards) {
        let actions =
            scraper::Selector::parse(".showcase-actions a[href$='.pdf']")
                .unwrap();
        let outputs =
            scraper::Selector::parse("details a[href$='.pdf']").unwrap();
        let visible = card.select(&actions).collect::<Vec<_>>();
        let all = card.select(&outputs).collect::<Vec<_>>();
        assert_eq!(
            visible.len(),
            all.len(),
            "every PDF edition must be visible on {}",
            card.value().attr("data-showcase").unwrap()
        );
        if all.len() > 1 {
            for link in visible {
                assert_ne!(
                    link.text().collect::<String>(),
                    "PDF",
                    "multiple PDFs need distinct titles"
                );
            }
        }
    }
    assert!(html.contains("artifacts/dprod/"));
    assert!(html.contains("DPROD: native ontology and SHACL"));
    assert!(!html.contains("artifacts/rdf11/"));
    assert!(!html.contains("W3C RDF 1.1 N-Triples"));
    assert!(html.contains("1 known issue in the source document"));
    assert!(!html.contains("expected source diagnostic"));
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
