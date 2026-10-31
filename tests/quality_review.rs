use grapharch::publication::{
    assembly::Assembler,
    collect::{Collector, RdfFile},
    dataset::Dataset,
    plan::TargetPlanner,
    render::{PublishOutcome, Publisher},
    writer::GraphWriter,
};

#[tokio::test]
async fn competing_annotations_survive_as_pinned_inline_and_addendum_warnings()
-> anyhow::Result<()> {
    let source = tempfile::tempdir()?;
    let primary = source.path().join("primary.ttl");
    let secondary = source.path().join("secondary.ttl");
    tokio::fs::write(
        &primary,
        format!(
            "{}\nex:subject <http://www.w3.org/2000/01/rdf-schema#label> \
             \"Primary label\"@en, \"Libellé français\"@fr ; dct:description \
             \"Primary definition\"@en .\n",
            include_str!("../examples/specification/project.ttl")
        ),
    )
    .await?;
    tokio::fs::write(&secondary, "<urn:quality:missing> a <http://www.w3.org/2002/07/owl#Class> . <urn:specification:subject> <http://www.w3.org/2000/01/rdf-schema#label> \"Alternate label\"@en ; <http://purl.org/dc/terms/description> \"Alternate definition\"@en .").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    for (id, path) in [
        ("urn:quality:primary", primary),
        ("urn:quality:secondary", secondary),
    ] {
        RdfFile::new("urn:specification:project", id, path)?
            .collect(&writer)
            .await?;
    }
    let documentor =
        grapharch::publication::document::AnnotatedTermDocumentor::new(
            dataset.clone(),
            dataset.snapshot_for("urn:specification:project").await?,
            "urn:specification:project",
            oxrdf::NamedNode::new("urn:quality:missing")?,
            "en",
            std::sync::Arc::new(std::collections::BTreeMap::new()),
            std::sync::Arc::new(std::collections::HashSet::new()),
        )?
        .with_source_annotations();
    use grapharch::publication::document::Documentor;
    documentor.document(&writer).await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &oxrdf::NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    let replacement = source.path().join("secondary.ttl");
    tokio::fs::write(&replacement, "<urn:quality:missing> a <http://www.w3.org/2002/07/owl#Class> . <urn:specification:subject> <http://www.w3.org/2000/01/rdf-schema#label> \"Primary label\"@en ; <http://purl.org/dc/terms/description> \"Primary definition\"@en .").await?;
    RdfFile::new(
        "urn:specification:project",
        "urn:quality:secondary",
        replacement,
    )?
    .collect(&writer)
    .await?;
    let current_revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let current = TargetPlanner::new(dataset.clone(), current_revision)
        .resolve(
            "urn:specification:project",
            &oxrdf::NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let current_output = tempfile::tempdir()?;
    let PublishOutcome::Published(corrected) = Publisher::new(restored.clone())
        .render(&current, current_output.path())
        .await?
    else {
        panic!("publication required")
    };
    assert!(
        !corrected
            .manifest
            .diagnostics
            .iter()
            .any(|d| d.code == "competing-source-annotations"),
        "resolved disagreements must disappear from the next report"
    );
    let PublishOutcome::Published(publication) =
        Publisher::new(restored.clone())
            .render(&plan, output.path())
            .await?
    else {
        panic!("publication required")
    };
    assert!(
        publication
            .manifest
            .diagnostics
            .iter()
            .any(|d| d.code == "missing-source-definition"),
        "existing documentor warnings must join the same addendum"
    );
    let warnings: Vec<_> = publication
        .manifest
        .diagnostics
        .iter()
        .filter(|d| d.code == "competing-source-annotations")
        .collect();
    assert_eq!(
        warnings.len(),
        2,
        "English labels and definitions are review findings; the French \
         translation is not"
    );
    assert!(warnings.iter().any(|d| {
        d.message.contains("Primary label") &&
            d.message.contains("Alternate label") &&
            d.message.contains("primary.ttl") &&
            d.message.contains("secondary.ttl")
    }));
    assert!(
        !warnings
            .iter()
            .any(|d| d.message.contains("Libellé français"))
    );
    let mut statuses = restored.select("SELECT ?status WHERE { GRAPH <urn:grapharch:publications> { ?publication <https://ekgf.org/ontology/grapharch#status> ?status } }".into()).await?;
    let mut reviewed = false;
    while let Some(row) = statuses.next().await? {
        assert_ne!(
            row["status"].to_string(),
            "\"published-with-errors\"",
            "warnings must not mark a successful publication as an error"
        );
        reviewed |= row["status"].to_string() == "\"published-with-warnings\"";
    }
    assert!(reviewed);
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains(&format!(
        "Generated by <a href=\"https://ekgf.github.io/grapharch/\">Grapharch \
         {}</a>",
        env!("CARGO_PKG_VERSION")
    )));
    let document = scraper::Html::parse_document(&html);
    let report = document
        .select(&scraper::Selector::parse("#quality-issues").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        report
            .select(&scraper::Selector::parse("h2").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>(),
        "B. Quality issues"
    );
    let groups = report
        .select(&scraper::Selector::parse(".quality-issue-group").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        groups.len(),
        3,
        "findings must be grouped by warning type"
    );
    assert!(groups.iter().any(|group| {
        group
            .select(&scraper::Selector::parse("h3").unwrap())
            .any(|heading| {
                heading.text().collect::<String>() ==
                    "Missing English source definition"
            })
    }));
    assert!(html.contains("Quality issues"));
    assert!(
        html.contains("href=\"#quality-issues\""),
        "the addendum must be discoverable in the contents"
    );
    assert!(
        html.contains("markdown-alert-warning"),
        "affected resources need an inline warning"
    );
    assert!(html.contains("Alternate definition"));
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(
        typst.contains(
            "#counter(heading).update(1)\n#heading(level: 1)[Quality issues]"
        ),
        "quality report must follow appendix A as appendix B"
    );
    assert!(typst.contains(
        "#heading(level: 2, numbering: none)[#text(\"Missing English source \
         definition\")]"
    ));
    assert!(typst.contains("Alternate label"));
    assert!(typst.contains("#ga-callout(\"Warning\""));
    Ok(())
}
