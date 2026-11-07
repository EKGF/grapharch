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
