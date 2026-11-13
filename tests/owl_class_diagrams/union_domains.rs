use super::*;

#[tokio::test]
async fn explicit_union_domain_is_a_readable_association_source() -> Result<()>
{
    let body = diagram(r#"
ex:Core a owl:Class; rdfs:label "Profile".
ex:Collector a owl:Class; rdfs:label "Collector".
ex:Documentor a owl:Class; rdfs:label "Documentor".
ex:navigationProfile a owl:ObjectProperty; rdfs:label "navigation profile";
 rdfs:domain [ a owl:Class; owl:unionOf (ex:Collector ex:Documentor) ]; rdfs:range ex:Core.
"#).await?;
    let value: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = value["source"].as_str().unwrap();
    assert!(
        source.contains("Collector or Documentor"),
        "{source}"
    );
    assert!(source.contains("navigation profile"), "{source}");
    for raw in [
        "Anonymous OWL",
        "rdfs:domain",
        "rdfs:range",
        "rdfs:Resource",
        "Object property",
    ] {
        assert!(!source.contains(raw), "{source}");
    }
    assert_eq!(
        value["diagram"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                e["predicate"] == "http://www.w3.org/2002/07/owl#unionOf"
            })
            .count(),
        2
    );
    Ok(())
}

#[tokio::test]
async fn union_domain_with_an_additional_class_constraint_stays_explicit()
-> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class.
ex:Collector a owl:Class; rdfs:label "Collector".
ex:Documentor a owl:Class; rdfs:label "Documentor".
ex:navigationProfile a owl:ObjectProperty; rdfs:label "navigation profile";
 rdfs:domain [ a owl:Class; owl:unionOf (ex:Collector ex:Documentor); rdfs:subClassOf ex:Restricted ]; rdfs:range ex:Core.
"#).await?;
    let value: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = value["source"].as_str().unwrap();
    assert!(source.contains("Anonymous OWL"), "{source}");
    assert!(
        !source.contains("Collector or Documentor"),
        "{source}"
    );
    assert!(!source.contains("rdfs:Resource"), "{source}");
    Ok(())
}

#[tokio::test]
async fn incomplete_or_non_named_union_domains_stay_explicit() -> Result<()> {
    for expression in [
        "[ a owl:Class; owl:unionOf [ <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ex:Collector ] ]",
        "[ a owl:Class; owl:unionOf (ex:Collector [ a owl:Class ]) ]",
        "[ a owl:Class; owl:unionOf (ex:Collector ex:Documentor); owl:intersectionOf (ex:Collector ex:Documentor) ]",
        "[ a owl:Class; owl:unionOf (ex:Collector ex:Documentor), (ex:Documentor ex:Collector) ]",
        "[ a owl:Class; owl:unionOf (ex:Collector ex:Collector) ]",
    ] {
        let body = diagram(&format!(r#"
ex:Core a owl:Class.
ex:Collector a owl:Class; rdfs:label "Collector".
ex:Documentor a owl:Class; rdfs:label "Documentor".
ex:navigationProfile a owl:ObjectProperty; rdfs:label "navigation profile";
 rdfs:domain {expression}; rdfs:range ex:Core.
"#)).await?;
        let value = projection(&body)?;
        let source = value["source"].as_str().unwrap();
        assert!(source.contains("Anonymous OWL"), "{expression}: {source}");
        assert!(!source.contains("Collector or Documentor"), "{source}");
        assert!(!source.contains("rdfs:Resource"), "{source}");
    }
    Ok(())
}

#[tokio::test]
async fn cyclic_union_domain_stays_explicit() -> Result<()> {
    let body = diagram(
        r#"
ex:Core a owl:Class.
ex:Collector a owl:Class; rdfs:label "Collector".
ex:navigationProfile a owl:ObjectProperty; rdfs:label "navigation profile";
 rdfs:domain [ a owl:Class; owl:unionOf _:cycle ]; rdfs:range ex:Core.
_:cycle <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ex:Collector;
 <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest> _:cycle.
"#,
    )
    .await?;
    let value = projection(&body)?;
    let source = value["source"].as_str().unwrap();
    assert!(source.contains("Anonymous OWL"), "{source}");
    assert!(!source.contains("rdfs:Resource"), "{source}");
    Ok(())
}

fn projection(body: &str) -> Result<serde_json::Value> {
    Ok(serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?)
}

#[tokio::test]
async fn union_association_survives_source_removal_and_renders_html_and_pdf()
-> Result<()> {
    use grapharch::publication::{
        assembly::Assembler,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
    };
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let path = source.path().join("ontology.ttl");
    tokio::fs::write(&path, r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
ex:Core a owl:Class; rdfs:label "Publication profile".
ex:Collector a owl:Class; rdfs:label "Collector".
ex:Documentor a owl:Class; rdfs:label "Documentor".
ex:navigationProfile a owl:ObjectProperty; rdfs:label "Navigation publication profile";
 rdfs:domain [ a owl:Class; owl:unionOf (ex:Collector ex:Documentor) ]; rdfs:range ex:Core.
"#).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:union:p", "urn:union:source", path)?
        .collect(&writer)
        .await?;
    let doc = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:union:p").await?,
        "urn:union:p",
        NamedNode::new("https://example.org/Core")?,
        "en",
        Arc::new(BTreeMap::from([(
            "https://example.org/".into(),
            "ex".into(),
        )])),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    doc.document(&writer).await?;
    let config = source.path().join("config.ttl");
    tokio::fs::write(&config, format!(r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#>.
<urn:union:p> a ga:Project; ga:target <urn:union:html>, <urn:union:pdf>.
<urn:union:html> a ga:Target; ga:renderer ga:HtmlRenderer; ga:destination "html"; ga:scope <urn:union:s>; ga:profile <urn:union:f>.
<urn:union:pdf> a ga:Target; ga:renderer ga:TypstRenderer ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; ga:publisher <urn:grapharch:example-authority>; ga:destination "pdf"; ga:scope <urn:union:s>; ga:profile <urn:union:f>.
<urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <https://ekgf.org/ontology/grapharch#slug> "example" ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; ga:slug "example" .
<urn:union:s> a ga:Scope; ga:include <https://example.org/Core>.
<urn:union:f> a ga:Profile; ga:placement <urn:union:a>.
<urn:union:a> a ga:Placement; ga:component <urn:union:c>; ga:order 0.
<urn:union:c> a ga:Component; ga:fragment {}.
"#, doc.fragment_id())).await?;
    RdfFile::new("urn:union:p", "urn:union:config", config)?
        .collect(&writer)
        .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:union:p")
        .await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let html = planner
        .resolve(
            "urn:union:p",
            &NamedNode::new("urn:union:html")?,
            &writer,
        )
        .await?;
    let pdf = planner
        .resolve(
            "urn:union:p",
            &NamedNode::new("urn:union:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(html) = Publisher::new(restored.clone())
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&html, output.path())
        .await?
    else {
        panic!("HTML not published");
    };
    let text =
        tokio::fs::read_to_string(html.directory.join("index.html")).await?;
    let page = scraper::Html::parse_document(&text);
    let svg = page
        .select(
            &scraper::Selector::parse("figure.owl-class-diagram svg").unwrap(),
        )
        .next()
        .unwrap();
    let labels = svg
        .select(&scraper::Selector::parse("text").unwrap())
        .flat_map(|t| t.text())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        labels.contains("Collector or Documentor"),
        "{labels}"
    );
    assert!(
        labels.contains("Navigation publication") && labels.contains("profile"),
        "{labels}"
    );
    assert!(
        !labels.contains("Anonymous OWL") && !labels.contains("rdfs:Resource"),
        "{labels}"
    );
    let PublishOutcome::Published(pdf) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&pdf, output.path())
        .await?
    else {
        panic!("PDF not published");
    };
    let typst = tokio::fs::read_to_string(pdf.typst_source()).await?;
    assert!(
        typst.contains("Collector or Documentor"),
        "retained SVG must reach PDF"
    );
    assert!(
        tokio::fs::metadata(pdf.pdf().expect("published PDF edition"))
            .await?
            .len() >
            1000
    );
    if std::env::var_os("GRAPHARCH_UNION_PREVIEW").is_some() {
        tokio::fs::create_dir_all("output/union-domain-review").await?;
        tokio::fs::copy(
            pdf.pdf().expect("published PDF edition"),
            "output/union-domain-review/document.pdf",
        )
        .await?;
        tokio::fs::write("output/union-domain-review/index.html", text).await?;
        tokio::fs::copy(
            html.directory.join("style.css"),
            "output/union-domain-review/style.css",
        )
        .await?;
    }
    Ok(())
}
