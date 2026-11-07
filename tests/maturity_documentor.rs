use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{Documentor, MaturityModelDocumentor},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};

const SOURCE: &str = r#"
@prefix mm: <https://raw.githubusercontent.com/EKGF/ontology-maturity-model/main/maturity-model.ttl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:model> a mm:Model; rdfs:label "Maturity" .
<urn:pillar> a mm:Pillar; rdfs:label "Business"; mm:pillarInModel <urn:model> .
<urn:area> a mm:CapabilityArea; rdfs:label "Strategy"; mm:inPillar <urn:pillar> .
<urn:capability> a mm:Capability; rdfs:label "Vision"; mm:inArea <urn:area>; mm:capabilityNumber "A.1.1"; rdfs:comment "Communicate goals."; dct:description "A **shared** vision." .
<urn:level> a mm:Level; rdfs:label "Level 1"; mm:levelInModel <urn:model> .
"#;

#[tokio::test]
async fn maturity_hierarchy_is_generated_from_restored_kg_without_sources()
-> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("model.ttl");
    tokio::fs::write(&source, SOURCE).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:project", "urn:collector", source.clone())?
        .collect(&writer)
        .await?;
    let pinned = dataset.snapshot_for("urn:project").await?;
    drop(writer);
    worker.await??;
    let archive = temp.path().join("kg.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(source).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let (writer, worker) =
        GraphWriter::spawn(restored.clone(), 4, 1024 * 1024)?;
    let doc =
        MaturityModelDocumentor::new(restored.clone(), pinned, "urn:project")?;
    assert_eq!(doc.subjects().await?.len(), 5);
    doc.document(&writer).await?;
    let snapshot = restored.snapshot_for("urn:project").await?;
    let view = RevisionView::new(restored.clone(), snapshot.clone());
    let body = view
        .text(
            &doc.fragment_id(&NamedNode::new("urn:capability")?),
            "body",
        )
        .await?;
    assert!(body.contains("A.1.1"));
    assert!(body.contains("A **shared** vision."));
    assert!(body.contains("[Strategy](urn:area)"));
    assert!(body.contains("Communicate goals."));
    let model = view
        .text(
            &doc.fragment_id(&NamedNode::new("urn:model")?),
            "body",
        )
        .await?;
    assert!(model.contains("[Business](urn:pillar)"));
    assert!(model.contains("[Level 1](urn:level)"));
    let area = view
        .text(
            &doc.fragment_id(&NamedNode::new("urn:area")?),
            "body",
        )
        .await?;
    assert!(area.contains("[Vision](urn:capability)"));
    assert!(
        view.optional(
            &doc.fragment_id(&NamedNode::new("urn:capability")?),
            "inputRevision"
        )
        .await?
        .is_some()
    );
    doc.document(&writer).await?;
    assert_eq!(
        snapshot,
        restored.snapshot_for("urn:project").await?
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn invalid_maturity_hierarchy_fails_before_fragments_are_written()
-> Result<()> {
    for invalid in [
        SOURCE.replace("mm:inArea <urn:area>", "mm:inArea <urn:missing>"),
        SOURCE.replace("mm:inArea <urn:area>", "mm:inArea <urn:pillar>"),
        SOURCE.replace(
            "mm:inArea <urn:area>",
            "mm:inArea <urn:area>, <urn:pillar>",
        ),
        SOURCE.replace(
            "rdfs:label \"Vision\"",
            "rdfs:label \"Vision\", \"Ambiguous\"",
        ),
        SOURCE.replace("a mm:Capability;", "a mm:Capability, mm:Pillar;"),
    ] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("model.ttl");
        tokio::fs::write(&source, invalid).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        RdfFile::new("urn:project", "urn:collector", source)?
            .collect(&writer)
            .await?;
        let doc = MaturityModelDocumentor::new(
            dataset.clone(),
            dataset.snapshot_for("urn:project").await?,
            "urn:project",
        )?;
        assert!(doc.document(&writer).await.is_err());
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:project").await?,
        );
        assert!(
            view.optional(
                &doc.fragment_id(&NamedNode::new("urn:model")?),
                "body"
            )
            .await?
            .is_none()
        );
        drop(writer);
        worker.await??;
    }
    Ok(())
}

#[tokio::test]
async fn removed_maturity_subjects_retire_and_numeric_order_is_preserved()
-> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("model.ttl");
    let extra = r#"
<urn:second> a mm:Capability; rdfs:label "Second"; mm:inArea <urn:area>; mm:capabilityNumber "A.1.2" .
<urn:tenth> a mm:Capability; rdfs:label "Tenth"; mm:inArea <urn:area>; mm:capabilityNumber "A.1.10" .
"#;
    tokio::fs::write(&source, format!("{SOURCE}{extra}")).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let collector =
        RdfFile::new("urn:project", "urn:collector", source.clone())?;
    collector.collect(&writer).await?;
    let doc = MaturityModelDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:project").await?,
        "urn:project",
    )?;
    doc.document(&writer).await?;
    let before = dataset.snapshot_for("urn:project").await?;
    let view = RevisionView::new(dataset.clone(), before.clone());
    let area = view
        .text(
            &doc.fragment_id(&NamedNode::new("urn:area")?),
            "body",
        )
        .await?;
    assert!(area.find("A.1.2").unwrap() < area.find("A.1.10").unwrap());
    tokio::fs::write(source, SOURCE).await?;
    collector.collect(&writer).await?;
    MaturityModelDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:project").await?,
        "urn:project",
    )?
    .document(&writer)
    .await?;
    let fragment = doc.fragment_id(&NamedNode::new("urn:tenth")?);
    assert!(
        RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:project").await?
        )
        .optional(&fragment, "body")
        .await?
        .is_none()
    );
    assert!(view.optional(&fragment, "body").await?.is_some());
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
#[ignore = "acceptance against the sibling ekg-maturity source checkout"]
async fn real_maturity_metadata_is_documented_from_a_restored_snapshot()
-> Result<()> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let mut files = tokio::fs::read_dir("../ekg-maturity/metadata").await?;
    let mut count = 0;
    while let Some(file) = files.next_entry().await? {
        if file.path().extension().is_some_and(|v| v == "ttl") {
            RdfFile::new(
                "urn:maturity:acceptance",
                "urn:maturity:rdf",
                file.path(),
            )?
            .collect(&writer)
            .await?;
            count += 1;
        }
    }
    assert!(count > 10);
    let pinned = dataset.snapshot_for("urn:maturity:acceptance").await?;
    drop(writer);
    worker.await??;
    let temp = tempfile::tempdir()?;
    let archive = temp.path().join("maturity.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let (writer, worker) =
        GraphWriter::spawn(restored.clone(), 4, 1024 * 1024)?;
    let doc = MaturityModelDocumentor::new(
        restored.clone(),
        pinned,
        "urn:maturity:acceptance",
    )?;
    let subjects = doc.subjects().await?;
    assert!(subjects.len() > 50);
    doc.document(&writer).await?;
    let view = RevisionView::new(
        restored.clone(),
        restored.snapshot_for("urn:maturity:acceptance").await?,
    );
    for subject in &subjects {
        assert!(
            !view
                .text(&doc.fragment_id(subject), "body")
                .await?
                .is_empty()
        );
    }
    eprintln!(
        "Documented {} subjects from {count} RDF files",
        subjects.len()
    );
    drop(writer);
    worker.await??;
    Ok(())
}
