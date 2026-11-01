use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

async fn body(definitions: &str, context: Option<&str>) -> Result<String> {
    let root = tempfile::tempdir()?;
    let file = root.path().join("source.ttl");
    tokio::fs::write(&file, format!(r#"
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:test:term> a <http://www.w3.org/2002/07/owl#Class> ;
 rdfs:label "Example"@en ; dct:description "Example description."@en {definitions} .
"#)).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:test:project", "urn:test:collector", file)?
        .collect(&writer)
        .await?;
    let job = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:test:project").await?,
        "urn:test:project",
        NamedNode::new("urn:test:term")?,
        "en",
        Arc::new(BTreeMap::new()),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    let job = if let Some(context) = context {
        job.with_ontology_context(NamedNode::new(context)?)
    } else {
        job
    };
    job.document(&writer).await?;
    let archive = root.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let view = RevisionView::new(
        restored.clone(),
        restored.snapshot_for("urn:test:project").await?,
    );
    let body = view.text(&job.fragment_id(), "body").await?;
    let mut rows = restored.select("SELECT ?o WHERE { GRAPH ?g { <urn:test:term> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> ?o } }".into()).await?;
    assert_eq!(
        rows.next().await?.is_some(),
        !definitions.is_empty(),
        "source assertion retained"
    );
    drop(writer);
    worker.await??;
    Ok(body)
}

#[tokio::test]
async fn redundant_definition_origin_is_hidden_only_with_explicit_context()
-> Result<()> {
    let source = "; rdfs:isDefinedBy <urn:test:ontology>";
    let contextual = body(source, Some("urn:test:ontology")).await?;
    assert!(
        !contextual.contains("isDefinedBy"),
        "redundant origin row: {contextual}"
    );
    assert!(body(source, None).await?.contains("isDefinedBy"));
    Ok(())
}

#[tokio::test]
async fn external_and_multiple_origins_remain_visible() -> Result<()> {
    for source in [
        "; rdfs:isDefinedBy <urn:test:external>",
        "; rdfs:isDefinedBy <urn:test:ontology>, <urn:test:external>",
    ] {
        let body = body(source, Some("urn:test:ontology")).await?;
        assert!(
            body.contains("isDefinedBy") && body.contains("urn:test:external")
        );
        if source.contains(',') {
            assert!(body.contains("urn:test:ontology"));
        }
    }
    Ok(())
}

#[tokio::test]
async fn missing_origin_is_explicit_only_when_ontology_context_is_known()
-> Result<()> {
    let contextual = body("", Some("urn:test:ontology")).await?;
    assert!(
        contextual.contains("isDefinedBy") &&
            contextual.contains("Not declared in collected sources.")
    );
    assert!(!body("", None).await?.contains("Not declared"));
    Ok(())
}
