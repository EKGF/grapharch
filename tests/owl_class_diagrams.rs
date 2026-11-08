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

async fn diagram(source: &str) -> Result<String> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("ontology.ttl");
    tokio::fs::write(
        &path,
        format!(
            r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
{source}
"#
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:diagram:project", "urn:diagram:source", path)?
        .collect(&writer)
        .await?;
    let snapshot = dataset.snapshot_for("urn:diagram:project").await?;
    let documentor = AnnotatedTermDocumentor::new(
        dataset.clone(),
        snapshot,
        "urn:diagram:project",
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
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:diagram:project").await?,
    );
    let body = view.text(&documentor.fragment_id(), "body").await?;
    drop(writer);
    worker.await??;
    Ok(body)
}

#[tokio::test]
async fn owl_classes_have_generated_assertion_neighborhoods() -> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class ; rdfs:label "Core" ; rdfs:subClassOf ex:Parent .
ex:Child a owl:Class ; rdfs:subClassOf ex:Core .
ex:connects a owl:ObjectProperty ; rdfs:domain ex:Core, ex:Other ; rdfs:range ex:Target .
ex:required a <http://www.w3.org/ns/shacl#PropertyShape> ; <http://www.w3.org/ns/shacl#targetClass> ex:Core .
"#).await?;
    assert!(
        body.contains("```grapharch-class-diagram"),
        "{body}"
    );
    for expected in [
        "ex:Parent",
        "ex:Child",
        "ex:connects",
        "ex:Other",
        "ex:Target",
    ] {
        assert!(body.contains(expected), "missing {expected}");
    }
    assert!(body.contains("Multiple domains and ranges apply together"));
    assert!(!body.contains("ex:required"));
    Ok(())
}

#[tokio::test]
async fn isolated_class_still_has_a_diagram_but_properties_do_not() -> Result<()>
{
    assert!(
        diagram("ex:Core a owl:Class .")
            .await?
            .contains("```grapharch-class-diagram")
    );
    assert!(
        !diagram("ex:Core a owl:ObjectProperty .")
            .await?
            .contains("```grapharch-class-diagram")
    );
    Ok(())
}

#[tokio::test]
async fn restrictions_cycles_and_large_neighborhoods_are_explicit() -> Result<()>
{
    let body = diagram(r#"
ex:Core a owl:Class ; rdfs:subClassOf ex:Other, [ a owl:Restriction ; owl:onProperty ex:p ; owl:someValuesFrom ex:Target ] .
ex:Other a owl:Class ; rdfs:subClassOf ex:Core .
"#).await?;
    assert!(body.contains("owl:someValuesFrom"));
    assert!(body.contains("Anonymous OWL expression"));
    let source = "ex:Core a owl:Class .\n".to_owned() +
        &(0..30)
            .map(|i| {
                format!("ex:C{i} a owl:Class ; rdfs:subClassOf ex:Core .\n")
            })
            .collect::<String>();
    let body = diagram(&source).await?;
    assert_eq!(
        body.matches("```grapharch-class-diagram").count(),
        1
    );
    assert!(body.contains("Diagram limit"));
    Ok(())
}

#[tokio::test]
async fn website_diagrams_link_classes_and_survive_export_without_sources()
-> Result<()> {
    use grapharch::publication::{
        quick_build::BuildRequest,
        render::{PublishOutcome, Publisher},
    };
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("ontology.ttl"),
        r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
ex:Core a owl:Class ; rdfs:label "Core" ; rdfs:subClassOf ex:Parent .
ex:Parent a owl:Class ; rdfs:label "Parent" .
ex:p a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Parent .
"#,
    )
    .await?;
    let request = BuildRequest {
        source: source.path().into(),
        output: output.path().join("before"),
    };
    let (dataset, plan) = request.collect().await?;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, &output.path().join("after"))
        .await?
    else {
        panic!("published");
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let page = scraper::Html::parse_document(&html);
    assert_eq!(
        page.select(
            &scraper::Selector::parse("figure.owl-class-diagram").unwrap()
        )
        .count(),
        2
    );
    let links: Vec<_> = page
        .select(&scraper::Selector::parse("svg a").unwrap())
        .collect();
    assert!(!links.is_empty());
    for link in links {
        let href = link.value().attr("href").unwrap();
        let anchor = href.strip_prefix('#').expect("local class navigation");
        assert!(
            page.select(&scraper::Selector::parse("[id]").unwrap())
                .any(|element| element.value().id() == Some(anchor)),
            "broken {href}"
        );
    }
    let mut ids = HashSet::new();
    for node in page.select(&scraper::Selector::parse("[id]").unwrap()) {
        assert!(
            ids.insert(node.value().id().unwrap()),
            "duplicate document ID"
        );
    }
    assert!(!html.contains("language-grapharch-class-diagram"));
    Ok(())
}

#[tokio::test]
async fn list_expressions_and_cardinalities_remain_explicit() -> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class ; owl:equivalentClass [ owl:unionOf (ex:A ex:B) ] ;
 rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; owl:minCardinality 2 ] .
"#).await?;
    assert!(body.contains("owl:unionOf member"));
    assert!(body.contains("ex:A"));
    assert!(body.contains("ex:B"));
    assert!(body.contains("owl:minCardinality"));
    assert!(body.contains("Literal value"));
    assert!(!body.contains("urn:grapharch:skolem:"));
    Ok(())
}

#[tokio::test]
async fn different_anonymous_restrictions_have_distinguishable_captions()
-> Result<()> {
    let body = diagram(r#"ex:Core a owl:Class ; rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; owl:someValuesFrom ex:A ], [ a owl:Restriction ; owl:onProperty ex:q ; owl:someValuesFrom ex:B ] ."#).await?;
    assert!(body.contains("Anonymous OWL expression 1"));
    assert!(body.contains("Anonymous OWL expression 2"));
    Ok(())
}

#[tokio::test]
async fn a_small_neighborhood_keeps_property_domain_and_range_in_one_diagram()
-> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class .
ex:StandardsRelease a owl:Class ; owl:disjointWith ex:Core .
ex:artifactChecksum a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:artifactMediaType a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:artifactUrl a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#anyURI> .
ex:publicationArtifact a owl:ObjectProperty ; rdfs:range ex:Core .
"#).await?;
    assert_eq!(
        body.matches("```grapharch-class-diagram").count(),
        1,
        "eight related assertions must not be split into detached panels"
    );
    Ok(())
}

#[tokio::test]
async fn oversized_property_groups_are_omitted_whole() -> Result<()> {
    let mut source = "ex:Core a owl:Class . ex:p a owl:ObjectProperty ; \
                      rdfs:domain ex:Core .\n"
        .to_owned();
    for i in 0..100 {
        source.push_str(&format!("ex:p rdfs:range ex:Target{i} .\n"));
    }
    let body = diagram(&source).await?;
    assert!(body.contains("Diagram limit"));
    assert!(
        !body.contains("\"label\":\"ex:p\""),
        "a partial property group would misrepresent its domains/ranges"
    );
    Ok(())
}

#[tokio::test]
async fn generated_layout_and_engine_identity_are_retained_with_the_assertions()
-> Result<()> {
    let body =
        diagram("ex:Core a owl:Class ; rdfs:subClassOf ex:Parent .").await?;
    assert!(
        body.contains("!pragma layout smetana"),
        "generated PlantUML must be retained"
    );
    assert!(
        body.contains("PlantUML version"),
        "renderer identity must be retained"
    );
    assert!(
        body.contains("<svg"),
        "restored publications must not invoke a layout engine"
    );
    Ok(())
}
