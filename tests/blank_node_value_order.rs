//! Repeated anonymous values are ordered by their content, never by their
//! blank-node identity, source revision or triple order, so table rows,
//! numbered query sections and their anchors are stable across builds.
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::{BlankNode, NamedNode, NamedOrBlankNode, Term, Triple},
    oxttl::{NTriplesSerializer, TurtleParser},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        sync::Arc,
    },
};

const DPROD: &str = include_str!("fixtures/shacl-sparql/dprod-schedule.ttl");
const DPROD_SHAPE: &str =
    "https://ekgf.org/dprod/spec/develop/contracts/shapes/ScheduleShape";

const ORDERED: &str = r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<urn:ordered> a sh:NodeShape ;
  rdfs:label "ordered shape" ;
  sh:sparql [ sh:order 2 ; sh:message "Alpha." ;
    sh:select "SELECT $this WHERE { $this <urn:a> ?a }" ] ;
  sh:sparql [ sh:order 1 ; sh:message "Omega." ;
    sh:select "SELECT $this WHERE { $this <urn:z> ?z }" ] ;
  sh:sparql [ sh:message "Unordered." ;
    sh:select "SELECT $this WHERE { $this <urn:u> ?u }" ] .
"#;

/// Rewrite Turtle as N-Triples with permuted blank-node labels and the
/// triples in a different order.
fn permuted(turtle: &str, rotate: usize) -> Result<String> {
    let mut triples = Vec::new();
    for triple in TurtleParser::new().for_slice(turtle.as_bytes()) {
        triples.push(triple?);
    }
    let mut labels = HashMap::new();
    for triple in &triples {
        for term in [Term::from(triple.subject.clone()), triple.object.clone()]
        {
            if let Term::BlankNode(blank) = term {
                let next = labels.len();
                labels.entry(blank).or_insert(next);
            }
        }
    }
    let count = labels.len();
    let relabel = |blank: &BlankNode| {
        BlankNode::new_unchecked(format!("b{}", count - labels[blank]))
    };
    let mut triples: Vec<_> = triples
        .into_iter()
        .map(|triple| {
            let subject = match &triple.subject {
                NamedOrBlankNode::BlankNode(blank) => relabel(blank).into(),
                subject => subject.clone(),
            };
            let object = match &triple.object {
                Term::BlankNode(blank) => relabel(blank).into(),
                object => object.clone(),
            };
            Triple::new(subject, triple.predicate, object)
        })
        .collect();
    triples.reverse();
    let rotate = rotate % triples.len();
    triples.rotate_left(rotate);
    let mut serializer = NTriplesSerializer::new().for_writer(Vec::new());
    for triple in &triples {
        serializer.serialize_triple(triple)?;
    }
    Ok(String::from_utf8(serializer.finish())?)
}

/// Document `subject` from one source collected under `source`.
async fn body(
    source: &str,
    file: &str,
    text: &str,
    subject: &str,
) -> Result<String> {
    let directory = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    let path = directory.path().join(file);
    tokio::fs::write(&path, text).await?;
    RdfFile::new("urn:p", source, path)?
        .collect(&writer)
        .await?;
    let documentor = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:p").await?,
        "urn:p",
        NamedNode::new(subject)?,
        "en",
        Arc::new(BTreeMap::new()),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    documentor.document(&writer).await?;
    let snapshot = dataset.snapshot_for("urn:p").await?;
    let body = RevisionView::new(dataset.clone(), snapshot)
        .text(&documentor.fragment_id(), "body")
        .await?;
    drop(writer);
    worker.await??;
    Ok(body)
}

fn captions(body: &str) -> Vec<String> {
    body.split("SPARQL query ")
        .skip(1)
        .filter_map(|section| {
            let (number, section) = section.split_once("\n\n")?;
            let caption = section.split_once("\n\n")?.0;
            Some(format!("{number}: {caption}"))
        })
        .collect()
}

#[tokio::test]
async fn anonymous_values_ignore_blank_identity_and_triple_order() -> Result<()>
{
    let reference =
        body("urn:source-a", "schedule.ttl", DPROD, DPROD_SHAPE).await?;
    assert_eq!(captions(&reference).len(), 4, "{reference}");
    for rotate in [0, 7, 31] {
        let variant = body(
            &format!("urn:source-{rotate}"),
            "schedule.nt",
            &permuted(DPROD, rotate)?,
            DPROD_SHAPE,
        )
        .await?;
        assert_eq!(reference, variant, "rotation {rotate}");
    }
    Ok(())
}

#[tokio::test]
async fn query_sections_follow_order_then_content() -> Result<()> {
    let body = body("urn:o", "ordered.ttl", ORDERED, "urn:ordered").await?;
    assert_eq!(captions(&body), [
        "1: Omega.",
        "2: Alpha.",
        "3: Unordered."
    ]);
    let dprod = body_captions().await?;
    assert_eq!(dprod, {
        let mut sorted = dprod.clone();
        sorted.sort_by(|a, b| a[3..].cmp(&b[3..]));
        sorted
    });
    Ok(())
}

/// The DPROD schedule queries carry no `sh:order`, so their message text
/// orders them.
async fn body_captions() -> Result<Vec<String>> {
    Ok(captions(
        &body("urn:d", "schedule.ttl", DPROD, DPROD_SHAPE).await?,
    ))
}

const CLASS: &str = r#"
@prefix ex: <https://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:Core a owl:Class ; rdfs:label "Core" ;
  rdfs:subClassOf ex:Parent ,
    [ a owl:Restriction ; owl:onProperty ex:alpha ; owl:someValuesFrom ex:A ] ,
    [ a owl:Restriction ; owl:onProperty ex:beta ; owl:allValuesFrom ex:B ] ,
    [ a owl:Restriction ; owl:onProperty ex:gamma ;
      owl:someValuesFrom [ a owl:Restriction ; owl:onProperty ex:delta ;
        owl:someValuesFrom ex:D ] ] ;
  owl:equivalentClass [ owl:unionOf ( ex:A ex:B ) ] .
ex:CoreShape a sh:NodeShape ; sh:targetClass ex:Core ;
  sh:property [ sh:path ex:name ; sh:datatype xsd:string ; sh:maxCount 1 ] ,
    [ sh:path ex:code ; sh:datatype xsd:string ; sh:minCount 1 ] ,
    [ sh:path ex:part ; sh:class ex:Part ] .
"#;

#[tokio::test]
async fn class_diagrams_ignore_blank_identity_and_triple_order() -> Result<()> {
    let subject = "https://example.org/Core";
    let reference = body("urn:class-a", "class.ttl", CLASS, subject).await?;
    assert!(
        reference.contains("Anonymous OWL restriction"),
        "{reference}"
    );
    assert!(
        reference.contains("property-shape-"),
        "{reference}"
    );
    assert!(!reference.contains("skolem"), "{reference}");
    for rotate in [0, 5, 23] {
        let variant = body(
            &format!("urn:class-{rotate}"),
            "class.nt",
            &permuted(CLASS, rotate)?,
            subject,
        )
        .await?;
        assert_eq!(reference, variant, "rotation {rotate}");
    }
    Ok(())
}
