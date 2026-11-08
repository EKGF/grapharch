use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

#[tokio::test]
async fn actual_schedule_shape_survives_export_and_renders_html_and_pdf()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let class = "https://ekgf.org/dprod/spec/develop/Schedule";
    let shape =
        "https://ekgf.org/dprod/spec/develop/contracts/shapes/ScheduleShape";
    let path = source.path().join("schedule.ttl");
    tokio::fs::write(
        &path,
        format!(
            "{}\n<{class}> a <http://www.w3.org/2002/07/owl#Class> .",
            include_str!("fixtures/shacl-sparql/dprod-schedule.ttl")
        ),
    )
    .await?;
    RdfFile::new("urn:validation:p", "urn:validation:source", path)?
        .collect(&writer)
        .await?;
    let prefixes = Arc::new(BTreeMap::from([
        (
            "https://ekgf.org/dprod/spec/develop/".into(),
            "dprod".into(),
        ),
        (
            "https://ekgf.org/dprod/spec/develop/contracts/shapes/".into(),
            "shapes".into(),
        ),
        ("http://purl.org/dc/terms/".into(), "dct".into()),
        ("http://www.w3.org/ns/shacl#".into(), "sh".into()),
    ]));
    let snapshot = dataset.snapshot_for("urn:validation:p").await?;
    let class_doc = AnnotatedTermDocumentor::new(
        dataset.clone(),
        snapshot.clone(),
        "urn:validation:p",
        NamedNode::new(class)?,
        "en",
        prefixes.clone(),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    let shape_doc = AnnotatedTermDocumentor::new(
        dataset.clone(),
        snapshot,
        "urn:validation:p",
        NamedNode::new(shape)?,
        "en",
        prefixes,
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    tokio::fs::remove_dir_all(source.path()).await?;
    class_doc.document(&writer).await?;
    shape_doc.document(&writer).await?;
    let config = output.path().join("project.ttl");
    tokio::fs::write(&config, format!(r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:validation:p> a ga:Project;ga:target <urn:validation:html>,<urn:validation:pdf>.
<urn:validation:html> a ga:Target;ga:renderer "html";ga:destination "html";ga:scope <urn:validation:s>;ga:profile <urn:validation:f>.
<urn:validation:pdf> a ga:Target;ga:renderer "typst";ga:destination "pdf";ga:scope <urn:validation:s>;ga:profile <urn:validation:f>.
<urn:validation:s> a ga:Scope;ga:include <{class}>,<{shape}>.
<urn:validation:f> a ga:Profile;ga:placement <urn:validation:a>,<urn:validation:b>.
<urn:validation:a> a ga:Placement;ga:component <urn:validation:ca>;ga:order 0.
<urn:validation:b> a ga:Placement;ga:component <urn:validation:cb>;ga:order 1.
<urn:validation:ca> a ga:Component;ga:fragment {}.
<urn:validation:cb> a ga:Component;ga:fragment {}.
"#, class_doc.fragment_id(), shape_doc.fragment_id())).await?;
    RdfFile::new(
        "urn:validation:p",
        "urn:validation:config",
        config.clone(),
    )?
    .collect(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:validation:p")
        .await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let html = planner
        .resolve(
            "urn:validation:p",
            &NamedNode::new("urn:validation:html")?,
            &writer,
        )
        .await?;
    let pdf = planner
        .resolve(
            "urn:validation:p",
            &NamedNode::new("urn:validation:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(config).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(html) = Publisher::new(restored.clone())
        .render(&html, output.path())
        .await?
    else {
        panic!("HTML not published")
    };
    let text =
        tokio::fs::read_to_string(html.directory.join("index.html")).await?;
    assert!(
        text.contains("SHACL validation views") &&
            text.contains("shapes:ScheduleShape")
    );
    assert!(
        text.contains("dct:conformsTo") &&
            text.contains("sh:or") &&
            text.contains("sh:sparql")
    );
    let PublishOutcome::Published(pdf) =
        Publisher::new(restored).render(&pdf, output.path()).await?
    else {
        panic!("PDF not published")
    };
    if std::env::var_os("GRAPHARCH_SHACL_VIEW_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/shacl-class-view").await?;
        tokio::fs::copy(
            pdf.directory.join("document.pdf"),
            "output/shacl-class-view/document.pdf",
        )
        .await?;
        tokio::fs::write("output/shacl-class-view/index.html", text).await?;
    }
    Ok(())
}

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
    assert_eq!(body.matches("```grapharch-shacl-view").count(), 2);
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
