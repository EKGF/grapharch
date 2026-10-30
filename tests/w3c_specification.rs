#[path = "support/mod.rs"]
mod specification_assertions;

use grapharch::publication::{
    assembly::Assembler,
    collect::{Collector, W3cSpecificationFile},
    dataset::Dataset,
    plan::TargetPlanner,
    render::{PublishOutcome, Publisher},
    writer::GraphWriter,
};

#[tokio::test]
async fn unsupported_published_markup_cannot_replace_a_collected_revision()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source.html");
    let original =
        tokio::fs::read_to_string("examples/w3c-rdf12-n-triples/source.html")
            .await?;
    tokio::fs::write(&source, &original).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let collector = W3cSpecificationFile::new(
        "urn:w3c:project",
        "urn:w3c:collector",
        source.clone(),
        "https://www.w3.org/TR/2026/WD-rdf12-n-triples-20261023/",
    )?;
    collector.collect(&writer).await?;
    let previous = dataset.snapshot().await?;
    let invalid = original.replace(
        "id=\"sec-intro\"",
        "id=\"sec-intro\" onclick=\"alert(1)\"",
    );
    // An event on a section wrapper is outside its body; exercise the actual
    // fragment boundary rather than trusting the wrapper to be rendered.
    let invalid = invalid.replace(
        "id=\"x2-4-iris\"",
        "id=\"x2-4-iris\" onclick=\"alert(1)\"",
    );
    assert_ne!(invalid, original);
    tokio::fs::write(source, invalid).await?;
    assert!(collector.collect(&writer).await.is_err());
    assert_eq!(dataset.snapshot().await?, previous);
    assert!(dataset.ensure_ready("urn:w3c:project").await.is_err());
    drop(writer);
    worker.await??;
    Ok(())
}

#[test]
fn reference_inputs_match_their_recorded_digests() -> anyhow::Result<()> {
    use {
        base64::{Engine, engine::general_purpose::STANDARD},
        sha2::{Digest, Sha256},
    };
    let metadata: serde_yaml::Value = serde_yaml::from_str(include_str!(
        "../examples/w3c-rdf12-n-triples/reference.yaml"
    ))?;
    for (file, field) in [
        ("source.html", "source_sha256"),
        ("w3c-logo.png", "logo_sha256"),
        ("w3c-logo.svg", "logo_source_sha256"),
    ] {
        let bytes =
            std::fs::read(format!("examples/w3c-rdf12-n-triples/{file}"))?;
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            metadata[field].as_str().unwrap()
        );
    }
    let authority =
        include_str!("../examples/w3c-rdf12-n-triples/authority.ttl");
    let encoded = authority
        .split("data:image/png;base64,")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        STANDARD.decode(encoded)?,
        std::fs::read("examples/w3c-rdf12-n-triples/w3c-logo.png")?
    );
    Ok(())
}

#[tokio::test]
async fn rdf12_publication_preserves_modern_metadata_and_document_structure()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("rdf12.html");
    tokio::fs::copy(
        "examples/w3c-rdf12-n-triples/source.html",
        &source,
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let project = "urn:w3c-rdf12:project";
    let collector = W3cSpecificationFile::new(
        project,
        "urn:w3c-rdf12:collector",
        source.clone(),
        "https://www.w3.org/TR/2026/WD-rdf12-n-triples-20261023/",
    )?;
    collector.collect(&writer).await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, &collector.target_id(), &writer)
        .await?;
    drop(writer);
    worker.await??;
    let archive = temp.path().join("rdf12.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(source).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let source_revision = restored.snapshot_for(project).await?;
    let strict = Publisher::new(restored.clone())
        .render(&plan, temp.path())
        .await;
    assert!(
        format!(
            "{:#}",
            strict.err().expect("upstream defect must fail strictly")
        )
        .contains("must-signal-errors")
    );
    let outcome = Publisher::new(restored.clone())
        .with_failure_mode(
            grapharch::publication::diagnostics::FailureMode::KeepGoing,
        )
        .render(&plan, temp.path())
        .await?;
    let PublishOutcome::Published(publication) = outcome else {
        panic!("expected diagnostic preview")
    };
    assert!(
        publication
            .manifest
            .diagnostics
            .iter()
            .any(|error| error.reference == "#must-signal-errors")
    );
    assert_eq!(
        source_revision,
        restored.snapshot_for(project).await?
    );
    let mut errors = restored.select(r#"SELECT ?diagnostic ?fragment WHERE { GRAPH <urn:grapharch:publications> { ?job <https://ekgf.org/ontology/grapharch#diagnostic> ?diagnostic . ?fragment <https://ekgf.org/ontology/grapharch#subject> ?diagnostic ; <https://ekgf.org/ontology/grapharch#format> "text/markdown" } }"#.into()).await?;
    assert!(errors.next().await?.is_some());
    let html =
        tokio::fs::read_to_string(temp.path().join("specification/index.html"))
            .await?;
    for expected in [
        "RDF 1.2 N-Triples",
        "W3C Working Draft",
        "23 October 2026",
        "More details about this document",
        "History:",
        "Commit history",
        "Latest Recommendation:",
        "Feedback:",
        "Gregg Kellogg",
        "Dominik Tomaszuk",
        "Former editors:",
        "David Beckett",
        "Version Announcement",
        "Triple Terms",
        "Set of Documents",
        "Privacy Considerations",
        "Security Considerations",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    specification_assertions::assert_specification_scripts(&html);
    let mut constructs = restored.select("SELECT DISTINCT ?type WHERE { VALUES ?type { <https://ekgf.org/ontology/grapharch#Section> <https://ekgf.org/ontology/grapharch#DefinedTerm> <https://ekgf.org/ontology/grapharch#Example> <https://ekgf.org/ontology/grapharch#BibliographicEntry> } GRAPH ?graph { ?thing a ?type } }".into()).await?;
    let mut kinds = 0;
    while constructs.next().await?.is_some() {
        kinds += 1;
    }
    assert_eq!(kinds, 4);
    let source_doc = scraper::Html::parse_document(include_str!(
        "../examples/w3c-rdf12-n-triples/source.html"
    ));
    let rendered = scraper::Html::parse_document(&html);
    assert!(
        rendered
            .select(
                &scraper::Selector::parse("header details[open] summary")
                    .unwrap()
            )
            .next()
            .is_some()
    );
    assert!(
        rendered
            .select(
                &scraper::Selector::parse(
                    ".table-scroll[tabindex=\"0\"] table.grammar"
                )
                .unwrap()
            )
            .next()
            .is_some()
    );
    let example = rendered
        .select(
            &scraper::Selector::parse(
                ".fragment[data-format='text/html'] #ex-comments",
            )
            .unwrap(),
        )
        .next()
        .expect("native fragments must retain their format for scoped styling");
    let marker = example
        .select(&scraper::Selector::parse(".marker").unwrap())
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert_eq!(
        marker.split_whitespace().collect::<Vec<_>>().join(" "),
        "Example 1: Use of comments in N-Triples"
    );
    assert!(
        rendered
            .select(
                &scraper::Selector::parse(
                    ".header-wrapper > a.self-link[href]"
                )
                .unwrap()
            )
            .next()
            .is_some(),
        "native heading permalink anchors must survive collection and KG \
         restoration"
    );
    let toc = rendered
        .select(&scraper::Selector::parse("#toc").unwrap())
        .next()
        .unwrap();
    let labels = toc.text().collect::<String>();
    assert!(
        labels.contains("Abstract") &&
            labels.contains("Status of This Document") &&
            labels.contains("Set of Documents")
    );
    for selector in ["pre", "dfn", "table td", "dl.bibliography dt"] {
        let selector = scraper::Selector::parse(selector).unwrap();
        let text = |doc: &scraper::Html| {
            doc.select(&selector)
                .map(|node| node.text().collect::<String>())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            text(&rendered),
            text(&source_doc),
            "RDF 1.2 construct content changed"
        );
    }
    Ok(())
}
