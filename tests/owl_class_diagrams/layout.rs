use super::*;

#[derive(serde::Deserialize)]
struct Drawing {
    svg:    String,
    source: String,
}

async fn drawing(source: &str) -> Result<Drawing> {
    let body = diagram(source).await?;
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    Ok(serde_json::from_str(json)?)
}

fn bounds(document: &roxmltree::Document<'_>, label: &str) -> [f64; 4] {
    let entity = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("entity") &&
                node.descendants().any(|text| {
                    text.is_element() &&
                        text.tag_name().name() == "text" &&
                        text.text() == Some(label)
                })
        })
        .unwrap_or_else(|| panic!("missing entity {label}"));
    let rect = entity
        .descendants()
        .find(|node| node.tag_name().name() == "rect")
        .unwrap();
    ["x", "y", "width", "height"]
        .map(|name| rect.attribute(name).unwrap().parse().unwrap())
}

#[tokio::test]
async fn superclasses_above_subclasses_and_associations_beside_focal_class()
-> Result<()> {
    let retained = drawing(
        r#"
ex:Core a owl:Class ; rdfs:subClassOf ex:Parent, ex:OtherParent .
ex:Parent a owl:Class . ex:OtherParent a owl:Class .
ex:Child a owl:Class ; rdfs:subClassOf ex:Core .
ex:Target a owl:Class .
ex:connects a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Target .
"#,
    )
    .await?;
    let svg = roxmltree::Document::parse(&retained.svg)?;
    let core = bounds(&svg, "Core");
    for label in ["Parent", "OtherParent"] {
        let parent = bounds(&svg, label);
        assert!(
            parent[1] + parent[3] < core[1],
            "{label} must be above Core: {parent:?} / {core:?}"
        );
    }
    let child = bounds(&svg, "Child");
    assert!(
        core[1] + core[3] < child[1],
        "Child must be below Core"
    );
    let target = bounds(&svg, "Target");
    assert!(
        target[0] > core[0] + core[2],
        "Target should be beside Core"
    );
    assert!(
        (target[1] + target[3] / 2.0 - core[1] - core[3] / 2.0).abs() < core[3],
        "association should share the focal rank"
    );
    assert!(retained.source.contains("-up-|>"));
    assert!(
        !retained.source.contains("rdfs:"),
        "UML generalization needs no redundant RDF predicate label"
    );
    assert!(!svg.descendants().any(|node| {
        node.is_element() &&
            node.tag_name().name() == "text" &&
            node.text() == Some("subClassOf")
    }));
    Ok(())
}

#[tokio::test]
async fn cyclic_inheritance_keeps_both_arrows_without_impossible_vertical_hints()
-> Result<()> {
    let retained = drawing(
        "ex:Core a owl:Class ; rdfs:subClassOf ex:Peer . ex:Peer a owl:Class \
         ; rdfs:subClassOf ex:Core .",
    )
    .await?;
    assert_eq!(retained.source.matches("--|>").count(), 2);
    assert!(!retained.source.contains("-up-|>"));
    assert!(retained.svg.contains("data-link-type=\"extension\""));
    Ok(())
}

#[tokio::test]
async fn named_external_superclasses_keep_vertical_inheritance_without_invented_types()
-> Result<()> {
    let retained =
        drawing("ex:Core a owl:Class ; rdfs:subClassOf ex:External .").await?;
    let svg = roxmltree::Document::parse(&retained.svg)?;
    let core = bounds(&svg, "Core");
    let parent = bounds(&svg, "External");
    assert!(
        parent[1] + parent[3] < core[1],
        "External superclass must be above Core: {parent:?} / {core:?}"
    );
    assert!(retained.source.contains("-up-|>"));
    assert!(!retained.source.contains("subClassOf"));
    assert!(
        retained.source.contains("<<Resource>>"),
        "Missing source typing must not be replaced by an invented owl:Class \
         assertion"
    );
    Ok(())
}
