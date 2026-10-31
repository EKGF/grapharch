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
    tokio::fs::write(&secondary, "<urn:specification:subject> <http://www.w3.org/2000/01/rdf-schema#label> \"Alternate label\"@en ; <http://purl.org/dc/terms/description> \"Alternate definition\"@en .").await?;
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
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
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
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("Quality issues"));
    assert!(
        html.contains("markdown-alert-warning"),
        "affected resources need an inline warning"
    );
    assert!(html.contains("Alternate definition"));
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("Quality issues"));
    assert!(typst.contains("Alternate label"));
    assert!(typst.contains("#ga-callout(\"Warning\""));
    Ok(())
}
