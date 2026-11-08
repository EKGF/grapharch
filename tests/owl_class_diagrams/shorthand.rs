use super::*;

#[derive(serde::Deserialize)]
struct Retained {
    source:  String,
    svg:     String,
    diagram: Facts,
}
#[derive(serde::Deserialize)]
struct Facts {
    edges: Vec<Fact>,
}
#[derive(serde::Deserialize)]
struct Fact {
    from:      Resource,
    predicate: String,
    to:        Resource,
}
#[derive(serde::Deserialize)]
struct Resource {
    id: String,
}
fn retained(body: &str) -> Result<Retained> {
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    Ok(serde_json::from_str(json)?)
}
#[tokio::test]
async fn unambiguous_properties_become_linked_attributes_and_associations()
-> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class . ex:Target a owl:Class .
ex:name a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> .
ex:connects a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Target .
"#).await?;
    let drawing = retained(&body)?;
    assert_eq!(
        drawing.diagram.edges.len(),
        4,
        "original assertions remain exact"
    );
    for (property, predicate, target) in [
        ("name", "domain", "https://example.org/Core"),
        (
            "name",
            "range",
            "http://www.w3.org/2001/XMLSchema#string",
        ),
        ("connects", "domain", "https://example.org/Core"),
        ("connects", "range", "https://example.org/Target"),
    ] {
        assert!(drawing.diagram.edges.iter().any(|e| {
            e.from.id == format!("https://example.org/{property}") &&
                e.predicate ==
                    format!(
                        "http://www.w3.org/2000/01/rdf-schema#{predicate}"
                    ) &&
                e.to.id == target
        }));
    }
    assert!(
        drawing
            .source
            .contains("{field} ex:name : string [[[./grapharch-node/"),
        "{}",
        drawing.source
    );
    assert!(
        drawing.source.contains("--> n") &&
            drawing.source.contains(": ex:\\nconnects"),
        "{}",
        drawing.source
    );
    assert!(!drawing.source.contains("<<Object property>>"));
    assert!(!drawing.source.contains("<<Datatype property>>"));
    assert!(!drawing.source.contains("rdfs:\\ndomain"));
    assert!(
        drawing.svg.contains("./grapharch-node/n3"),
        "association retains property link"
    );
    assert!(
        drawing.svg.contains("./grapharch-node/n4"),
        "attribute retains property link"
    );
    Ok(())
}
#[tokio::test]
async fn ambiguous_or_referenced_properties_keep_explicit_assertions()
-> Result<()> {
    for source in [
        "ex:p a owl:AnnotationProperty ; rdfs:domain ex:Core, ex:Other ; \
         rdfs:range ex:Target .",
        "ex:p a owl:ObjectProperty ; rdfs:domain ex:Core, ex:Other ; \
         rdfs:range ex:Target .",
        "ex:p a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range \
         ex:Target, ex:Other .",
        "ex:p a owl:ObjectProperty ; rdfs:range ex:Core .",
        "ex:p a owl:DatatypeProperty ; rdfs:domain ex:Core ; rdfs:range [ \
         owl:unionOf (ex:A ex:B) ] .",
        "ex:p a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range \
         ex:Target . ex:Core rdfs:subClassOf [ a owl:Restriction ; \
         owl:onProperty ex:p ; owl:someValuesFrom ex:Target ] .",
    ] {
        let body = diagram(&format!(
            "ex:Core a owl:Class . ex:Other a owl:Class . ex:Target a \
             owl:Class . {source}"
        ))
        .await?;
        let drawing = retained(&body)?;
        assert!(
            drawing.source.contains("property>>"),
            "property must remain explicit: {}",
            drawing.source
        );
        assert!(!drawing.source.contains("{field}"));
    }
    Ok(())
}

#[tokio::test]
async fn datatype_without_range_does_not_invent_a_type_or_requiredness()
-> Result<()> {
    let body = diagram(
        "ex:Core a owl:Class . ex:p a owl:DatatypeProperty ; rdfs:domain \
         ex:Core .",
    )
    .await?;
    let drawing = retained(&body)?;
    assert_eq!(drawing.diagram.edges.len(), 1);
    assert!(drawing.source.contains("{field} ex:p [[["));
    assert!(!drawing.source.contains("{field} ex:p :"));
    assert!(!drawing.source.contains("[1]"));
    Ok(())
}

#[tokio::test]
async fn generic_rdfs_resource_is_only_hidden_in_the_drawing() -> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class ; rdfs:subClassOf rdfs:Resource, ex:Resource .
ex:Resource a owl:Class .
ex:generic a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range rdfs:Resource .
ex:ambiguous a owl:ObjectProperty ; rdfs:domain ex:Core, ex:Resource ; rdfs:range rdfs:Resource .
"#).await?;
    let drawing = retained(&body)?;
    assert_eq!(drawing.diagram.edges.len(), 7);
    assert!(drawing.diagram.edges.iter().any(|edge| {
        edge.to.id == "http://www.w3.org/2000/01/rdf-schema#Resource"
    }));
    assert!(
        !drawing.source.contains("rdfs:\\nResource"),
        "{}",
        drawing.source
    );
    assert!(drawing.source.contains("ex:\\nResource"));
    assert!(
        drawing
            .source
            .contains("{field} ex:generic [[[./grapharch-node/")
    );
    assert!(drawing.source.contains("ex:\\nambiguous"));
    assert!(drawing.source.contains("rdfs:\\ndomain"));
    Ok(())
}

#[tokio::test]
async fn annotation_and_generic_properties_use_declared_domain_to_range_associations()
-> Result<()> {
    for declaration in [
        "a owl:AnnotationProperty ;",
        "a owl:ObjectProperty, owl:DatatypeProperty ;",
        "a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> ;",
        "",
    ] {
        let body = diagram(&format!(r#"
ex:Core a owl:Class .
ex:evidencePredicate {declaration} rdfs:domain ex:Core ; rdfs:range <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .
"#)).await?;
        let drawing = retained(&body)?;
        assert_eq!(drawing.diagram.edges.len(), 2);
        assert!(
            drawing.source.contains(
                "n1 --> n0 [[./grapharch-node/n2]] : ex:\\nevidencePredicate"
            ),
            "{}",
            drawing.source
        );
        assert!(!drawing.source.contains("rdfs:\\ndomain"));
        assert!(!drawing.source.contains("rdfs:\\nrange"));
        assert!(!drawing.source.contains("{field}"));
        assert!(drawing.source.contains("Property"));
        assert!(!body.contains("Object property"));
    }
    Ok(())
}

#[tokio::test]
async fn object_property_with_only_domain_is_an_untyped_member() -> Result<()> {
    let body = diagram(
        "ex:Core a owl:Class . ex:domain a owl:ObjectProperty ; rdfs:domain \
         ex:Core .",
    )
    .await?;
    let drawing = retained(&body)?;
    assert_eq!(drawing.diagram.edges.len(), 1);
    assert!(
        drawing.source.contains("{field} ex:domain [[["),
        "{}",
        drawing.source
    );
    assert!(!drawing.source.contains("{field} ex:domain :"));
    Ok(())
}
