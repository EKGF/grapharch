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
    diagram_with_prefix(source, "ex").await
}

async fn diagram_with_prefix(source: &str, prefix: &str) -> Result<String> {
    diagram_with_focus(source, prefix, "Core").await
}

async fn diagram_with_focus(
    source: &str,
    prefix: &str,
    focus: &str,
) -> Result<String> {
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
        NamedNode::new(format!("https://example.org/{focus}"))?,
        "en",
        Arc::new(BTreeMap::from([(
            "https://example.org/".into(),
            prefix.into(),
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
    assert!(!body.contains("Multiple domains and ranges apply together"));
    assert!(!body.contains("OWL neighborhood diagrams"));
    assert!(!body.contains("ex:required"));
    Ok(())
}

#[tokio::test]
async fn isolated_class_still_has_a_diagram_but_properties_do_not() -> Result<()>
{
    let body = diagram("ex:Core a owl:Class .").await?;
    assert!(body.contains("```grapharch-class-diagram"));
    assert!(!body.contains("No direct class, domain or range relationships"));
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
    assert!(!body.contains("Diagram limit"));
    assert!(body.contains("\"limited\":true"));
    #[derive(serde::Deserialize)]
    struct Retained {
        diagram: Neighborhood,
    }
    #[derive(serde::Deserialize)]
    struct Neighborhood {
        edges: Vec<serde::de::IgnoredAny>,
    }
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: Retained = serde_json::from_str(json)?;
    assert_eq!(
        retained.diagram.edges.len(),
        5,
        "a high-degree class must stay a small navigable neighborhood"
    );
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
ex:Core a owl:Class ; rdfs:label "Core" ; rdfs:comment "The central class." ; rdfs:subClassOf ex:Parent, [ a owl:Restriction ; owl:onProperty ex:p ; owl:someValuesFrom ex:Parent ] .
ex:Parent a owl:Class ; rdfs:label "Parent" ; rdfs:comment "The parent class." .
ex:p a owl:ObjectProperty ; rdfs:label "Related parent" ; rdfs:comment "Relates a core instance to its parent." ; rdfs:domain ex:Core ; rdfs:range ex:Parent .
ex:name a owl:DatatypeProperty ; rdfs:label "Name" ; rdfs:comment "A human-readable name." ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:incoming a owl:ObjectProperty ; rdfs:label "delegates control to" ; rdfs:comment "Delegates control to a core instance." ; rdfs:range ex:Core .
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
    restored.import(archive.clone()).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
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
    assert!(html.contains("1..*") && html.contains("qualified"));
    assert!(html.contains("presentation placeholder"));
    assert!(html.contains("delegates control to"));
    assert!(html.contains("title=\"https://example.org/incoming\""));
    let store_path = output.path().join("retained-store");
    let disk = Dataset::open(store_path.clone()).await?;
    disk.import(archive).await?;
    drop(disk);
    let empty_path = output.path().join("no-layout-tools");
    tokio::fs::create_dir(&empty_path).await?;
    let rendered =
        tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
            .env("PATH", &empty_path)
            .env_remove("JAVA_HOME")
            .args(["publication", "render", "--store"])
            .arg(store_path)
            .args([
                "--plan",
                plan.id.as_str(),
                "--snapshot",
                plan.snapshot.as_str(),
                "--output-root",
            ])
            .arg(output.path().join("without-tools"))
            .output()
            .await?;
    assert!(
        rendered.status.success(),
        "restored rendering must not require PlantUML or Java: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    Ok(())
}

#[tokio::test]
async fn list_expressions_and_cardinalities_remain_explicit() -> Result<()> {
    // Check each semantic construct independently: the combined neighborhood
    // can exceed the layout budget with different platform font metrics.
    let union = diagram(
        r#"
ex:Core a owl:Class ; owl:equivalentClass [ owl:unionOf (ex:A ex:B) ] .
"#,
    )
    .await?;
    assert!(union.contains("owl:unionOf member"));
    assert!(union.contains("ex:A"));
    assert!(union.contains("ex:B"));
    let restriction = diagram(r#"
ex:Core a owl:Class ; rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; owl:minCardinality 2 ] .
"#).await?;
    assert!(restriction.contains("owl:minCardinality"));
    assert!(restriction.contains("Literal value"));
    assert!(!union.contains("urn:grapharch:skolem:"));
    assert!(!restriction.contains("urn:grapharch:skolem:"));
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
    assert!(!body.contains("Diagram limit"));
    assert!(body.contains("\"limited\":true"));
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

#[tokio::test]
async fn a_restriction_neighborhood_uses_a_balanced_engine_orientation()
-> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class ; rdfs:subClassOf ex:Designation, [ a owl:Restriction ; owl:onProperty ex:compliesWith ; owl:onClass ex:IdentificationScheme ; owl:minQualifiedCardinality 0 ] ; owl:disjointWith ex:Name .
ex:identifies a owl:ObjectProperty ; rdfs:domain ex:Core .
ex:isIdentifiedBy a owl:ObjectProperty ; rdfs:range ex:Core .
ex:IdentificationScheme a owl:Class .
"#).await?;
    #[derive(serde::Deserialize)]
    struct Retained {
        svg: String,
    }
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: Retained = serde_json::from_str(json)?;
    let svg = roxmltree::Document::parse(&retained.svg)?;
    let width: f64 = svg
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()?;
    let height: f64 = svg
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(3)
        .unwrap()
        .parse()?;
    assert!(
        height <= width * 1.25,
        "figure should be balanced rather than narrow and tall: \
         {width}x{height}"
    );
    assert!(
        width <= 1200.0,
        "focused fixture should remain bounded"
    );
    Ok(())
}

#[tokio::test]
async fn layout_preserves_complete_groups_instead_of_optimizing_for_phone_width()
-> Result<()> {
    let body = diagram_with_prefix(r#"
ex:Core a owl:Class ; rdfs:subClassOf ex:MembershipEvidence, ex:MembershipEvent .
ex:consentMembership a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Membership .
ex:withdrawnConsent a owl:ObjectProperty ; rdfs:domain ex:RecipientConsentWithdrawal ; rdfs:range ex:Core .
ex:capturedAt a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#dateTime> .
"#, "ekg-communication").await?;
    #[derive(serde::Deserialize)]
    struct Retained {
        svg: String,
    }
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: Retained = serde_json::from_str(json)?;
    let svg = roxmltree::Document::parse(&retained.svg)?;
    let width: f64 = svg
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()?;
    assert!(width > 0.0);
    assert!(
        !body.contains("Diagram limit"),
        "layout must not omit asserted groups"
    );
    assert!(body.contains("withdrawnConsent"));
    Ok(())
}

#[tokio::test]
async fn named_classes_use_uml_class_and_generalization_notation() -> Result<()>
{
    let body = diagram(
        "ex:Core a owl:Class ; rdfs:subClassOf ex:Parent . ex:Parent a \
         owl:Class . ex:p a owl:ObjectProperty ; rdfs:domain ex:Core ; \
         rdfs:range ex:Parent .",
    )
    .await?;
    #[derive(serde::Deserialize)]
    struct Retained {
        source: String,
    }
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: Retained = serde_json::from_str(json)?;
    assert!(
        retained.source.contains("class \"ex:"),
        "OWL classes need UML class declarations"
    );
    assert!(retained.source.contains("<<owl:Class>>"));
    assert!(
        retained.source.contains("n0 -up-|> n1"),
        "generalization must point from child to parent"
    );
    assert!(
        retained
            .source
            .contains("n0 -up-> n1 [[./grapharch-node/n2]]"),
        "single-domain properties use a linked association"
    );
    assert!(!retained.source.contains("*--"));
    Ok(())
}

#[tokio::test]
async fn publication_artifact_keeps_a_balanced_compact_layout_and_all_assertions()
-> Result<()> {
    let body = diagram_with_focus(r#"
ex:PublicationArtifact a owl:Class .
ex:StandardsRelease a owl:Class ; owl:disjointWith ex:PublicationArtifact .
ex:artifactChecksum a owl:DatatypeProperty ; rdfs:domain ex:PublicationArtifact ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:artifactMediaType a owl:DatatypeProperty ; rdfs:domain ex:PublicationArtifact ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:artifactUrl a owl:DatatypeProperty ; rdfs:domain ex:PublicationArtifact ; rdfs:range <http://www.w3.org/2001/XMLSchema#anyURI> .
ex:publicationArtifact a owl:ObjectProperty ; rdfs:range ex:PublicationArtifact .
"#, "ekg-artpub", "PublicationArtifact").await?;
    #[derive(serde::Deserialize)]
    struct Retained {
        svg:     String,
        source:  String,
        diagram: Semantic,
    }
    #[derive(serde::Deserialize)]
    struct Semantic {
        edges: Vec<serde::de::IgnoredAny>,
    }
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: Retained = serde_json::from_str(json)?;
    assert_eq!(retained.diagram.edges.len(), 8);
    for property in ["artifactChecksum", "artifactMediaType", "artifactUrl"] {
        assert!(
            retained
                .source
                .contains(&format!("{{field}} ekg-artpub:{property} :"))
        );
    }
    assert!(!body.contains("Diagram limit"));
    let svg = roxmltree::Document::parse(&retained.svg)?;
    let bounds = svg.root_element().attribute("viewBox").unwrap();
    let mut dimensions = bounds.split_whitespace().skip(2);
    let width: f64 = dimensions.next().unwrap().parse()?;
    let height: f64 = dimensions.next().unwrap().parse()?;
    assert!(
        height <= width * 1.25 && width <= 1200.0,
        "Compact attribute compartments should remain balanced: \
         {width}x{height}"
    );
    Ok(())
}

#[path = "owl_class_diagrams/shorthand.rs"]
mod shorthand;

#[path = "owl_class_diagrams/layout.rs"]
mod layout;

#[path = "owl_class_diagrams/constraints.rs"]
mod constraints;

#[path = "owl_class_diagrams/existentials.rs"]
mod existentials;

#[path = "owl_class_diagrams/range_only.rs"]
mod range_only;

#[path = "owl_class_diagrams/qualified_minimum.rs"]
mod qualified_minimum;

#[path = "owl_class_diagrams/qualified_exact.rs"]
mod qualified_exact;

#[path = "owl_class_diagrams/qualified_maximum.rs"]
mod qualified_maximum;

#[path = "owl_class_diagrams/qualified_data.rs"]
mod qualified_data;

#[path = "owl_class_diagrams/union_domains.rs"]
mod union_domains;
