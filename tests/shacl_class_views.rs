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

async fn view(source: &str) -> Result<String> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("source.ttl");
    tokio::fs::write(
        &path,
        format!(
            r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:Core a owl:Class .
{source}
"#
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        "urn:shacl:project",
        "urn:shacl:source",
        path.clone(),
    )?
    .collect(&writer)
    .await?;
    tokio::fs::remove_file(path).await?;
    let documentor = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:shacl:project").await?,
        "urn:shacl:project",
        NamedNode::new("https://example.org/Core")?,
        "en",
        Arc::new(BTreeMap::from([(
            "https://example.org/".into(),
            "ex".into(),
        )])),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    documentor.document(&writer).await?;
    let body = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:shacl:project").await?,
    )
    .text(&documentor.fragment_id(), "body")
    .await?;
    drop(writer);
    worker.await??;
    Ok(body)
}

#[tokio::test]
async fn exact_named_shapes_remain_distinct_and_keep_repeated_paths()
-> Result<()> {
    let body = view(r#"
ex:First a sh:NodeShape; sh:targetClass ex:Core; sh:closed true;
 sh:property [sh:path ex:name; sh:minCount 1; sh:maxCount 1; sh:datatype xsd:string],
 [sh:path ex:name; sh:minCount 2; sh:maxCount 1; sh:nodeKind sh:Literal].
ex:Second a sh:NodeShape; sh:targetClass ex:Core; sh:property [sh:path ex:owner; sh:class ex:Person].
ex:Unselected a sh:NodeShape; sh:targetClass ex:Other.
ex:Untyped sh:targetClass ex:Core.
ex:WrongType a sh:PropertyShape; sh:targetClass ex:Core.
"#).await?;
    assert!(body.contains("SHACL validation views"), "{body}");
    assert!(body.contains("ex:First") && body.contains("ex:Second"));
    assert!(body.contains("not merged") && body.contains("instances"));
    assert_eq!(body.matches("| [ex:name]").count(), 2);
    assert!(body.contains("minCount: 2") && body.contains("maxCount: 1"));
    assert!(body.contains("Declared counts conflict"));
    assert!(body.contains("sh:class") && body.contains("ex:Person"));
    assert!(
        !body.contains("ex:Unselected") &&
            !body.contains("ex:Untyped") &&
            !body.contains("ex:WrongType")
    );
    Ok(())
}

#[tokio::test]
async fn complex_constraints_are_references_not_invented_multiplicities()
-> Result<()> {
    let body = view(r#"
ex:Complex a sh:NodeShape; sh:targetClass ex:Core; sh:sparql [sh:select "SELECT $this WHERE {}"];
 sh:property [sh:path [sh:inversePath ex:owner]; sh:minCount 1],
 [sh:path ex:friend; sh:qualifiedValueShape [sh:class ex:Person]; sh:qualifiedMinCount 2].
"#).await?;
    assert!(body.contains("Complex path"), "{body}");
    assert!(
        body.contains("qualifiedMinCount") &&
            body.contains("Full shape definition")
    );
    assert!(body.contains("sh:sparql"));
    assert!(!body.contains("2..*") && !body.contains("0..*"));
    assert!(!view("").await?.contains("SHACL validation views"));
    Ok(())
}
