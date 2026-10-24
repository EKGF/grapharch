use grapharch::publication::{
    assembly::Assembler,
    collect::{Collector, W3cSpecificationFile},
    dataset::Dataset,
    plan::TargetPlanner,
    render::{PublishOutcome, Publisher},
    writer::GraphWriter,
};

#[tokio::test]
async fn published_n_triples_is_collected_as_queryable_things_and_rendered_from_restored_kg()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source.html");
    tokio::fs::copy("examples/w3c-n-triples/source.html", &source).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let project = "urn:w3c-example:project";
    let collector = W3cSpecificationFile::new(
        project,
        "urn:w3c-example:collector",
        source.clone(),
        "https://www.w3.org/TR/2014/REC-n-triples-20140225/",
    )?;
    collector.collect(&writer).await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, &collector.target_id(), &writer)
        .await?;
    drop(writer);
    worker.await??;
    let mut rows = dataset.select("SELECT (COUNT(DISTINCT ?section) AS ?count) WHERE { GRAPH ?graph { ?section a <https://ekgf.org/ontology/grapharch#Section> } }".into()).await?;
    let row = rows.next().await?.unwrap();
    let oxrdf::Term::Literal(count) = &row["count"] else {
        panic!("expected a count")
    };
    assert!(count.value().parse::<usize>()? >= 25);
    for class in ["DefinedTerm", "Example", "BibliographicEntry"] {
        let mut rows = dataset.select(format!("SELECT ?thing WHERE {{ GRAPH ?g {{ ?thing a <https://ekgf.org/ontology/grapharch#{class}> }} }} LIMIT 1")).await?;
        assert!(
            rows.next().await?.is_some(),
            "no collected {class}"
        );
    }
    let archive = temp.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(source).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let outcome = Publisher::new(restored).render(&plan, temp.path()).await?;
    assert!(matches!(outcome, PublishOutcome::Published(_)));
    let html =
        tokio::fs::read_to_string(temp.path().join("specification/index.html"))
            .await?;
    for expected in [
        "RDF 1.1 N-Triples",
        "W3C Recommendation",
        "25 February 2014",
        "Gavin Carothers",
        "Andy Seaborne",
        "David Beckett",
        "2.1 Simple Triples",
        "N-Triples Internet Media Type",
        "STRING_LITERAL_QUOTE",
        "<table",
        "<pre",
        "id=\"sec-introduction\"",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    assert!(
        html.find("Gavin Carothers").unwrap() <
            html.find("Andy Seaborne").unwrap(),
        "source editor ordering must survive collection"
    );
    let source_doc = scraper::Html::parse_document(include_str!(
        "../examples/w3c-n-triples/source.html"
    ));
    let rendered_doc = scraper::Html::parse_document(&html);
    assert!(
        rendered_doc
            .select(
                &scraper::Selector::parse(
                    ".table-scroll[tabindex=\"0\"] table.grammar"
                )
                .unwrap()
            )
            .next()
            .is_some(),
        "wide grammar tables need a keyboard-accessible scroll region"
    );
    for selector in ["pre", "dfn", "table td", "dl.bibliography dt"] {
        let selector = scraper::Selector::parse(selector).unwrap();
        let text = |document: &scraper::Html| {
            document
                .select(&selector)
                .map(|node| node.text().collect::<String>())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            text(&rendered_doc),
            text(&source_doc),
            "published construct content changed"
        );
    }
    assert!(!html.contains("<script"));
    Ok(())
}

#[tokio::test]
async fn unsupported_published_markup_cannot_replace_a_collected_revision()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source.html");
    let original =
        tokio::fs::read_to_string("examples/w3c-n-triples/source.html").await?;
    tokio::fs::write(&source, &original).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let collector = W3cSpecificationFile::new(
        "urn:w3c:project",
        "urn:w3c:collector",
        source.clone(),
        "https://www.w3.org/TR/2014/REC-n-triples-20140225/",
    )?;
    collector.collect(&writer).await?;
    let previous = dataset.snapshot().await?;
    let invalid = original.replace(
        "id=\"sec-introduction\"",
        "id=\"sec-introduction\" onclick=\"alert(1)\"",
    );
    // An event on a section wrapper is outside its body; exercise the actual
    // fragment boundary rather than trusting the wrapper to be rendered.
    let invalid = invalid.replace(
        "id=\"h3_sec-iri\"",
        "id=\"h3_sec-iri\" onclick=\"alert(1)\"",
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
        "../examples/w3c-n-triples/reference.yaml"
    ))?;
    for (file, field) in [
        ("source.html", "source_sha256"),
        ("w3c-logo.png", "logo_sha256"),
    ] {
        let bytes = std::fs::read(format!("examples/w3c-n-triples/{file}"))?;
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            metadata[field].as_str().unwrap()
        );
    }
    let authority = include_str!("../examples/w3c-n-triples/authority.ttl");
    let encoded = authority
        .split("data:image/png;base64,")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        STANDARD.decode(encoded)?,
        std::fs::read("examples/w3c-n-triples/w3c-logo.png")?
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
