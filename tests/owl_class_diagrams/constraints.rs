use super::*;

#[tokio::test]
async fn authored_shapes_enrich_one_class_neighborhood_without_merging_shapes()
-> Result<()> {
    let body = diagram(
        r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:Core a owl:Class ; rdfs:subClassOf ex:Base . ex:Base a owl:Class . ex:Target a owl:Class . ex:Other a owl:Class .
ex:parent a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Target .
ex:A a sh:NodeShape ; sh:targetClass ex:Core ; sh:property
 [ sh:path ex:name ; sh:datatype xsd:string ; sh:minCount 1 ; sh:maxCount 1 ],
 [ sh:path ex:additional ; sh:class ex:Other ; sh:maxCount 1 ],
 [ sh:path ex:parent ; sh:class ex:Target ; sh:minCount 1 ; sh:maxCount 3 ],
 [ sh:path [ sh:inversePath ex:back ] ; sh:minCount 9 ] .
ex:B a sh:NodeShape ; sh:targetClass ex:Core ; sh:property
 [ sh:path ex:parent ; sh:class ex:Target ; sh:maxCount 0 ] .
ex:Neighbor a sh:NodeShape ; sh:targetClass ex:Target ; sh:property
 [ sh:path ex:code ; sh:datatype xsd:string ] .
"#,
    )
    .await?;
    assert_eq!(
        body.matches("```grapharch-class-diagram").count(),
        1
    );
    assert!(!body.contains("```grapharch-shacl-view"));
    let source = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let retained: serde_json::Value = serde_json::from_str(source)?;
    let drawing = retained["source"].as_str().unwrap();
    assert!(!drawing.contains("SHACL"), "{drawing}");
    assert!(!drawing.contains("S1") && !drawing.contains("S2"));
    assert!(drawing.contains("ex:name") && drawing.contains("string"));
    assert!(!drawing.contains("1..3") && !drawing.contains("0..0"));
    assert!(
        source.contains("\"minimum\":1") && source.contains("\"maximum\":0")
    );
    assert!(!drawing.contains("9.."));
    assert!(drawing.contains("-right-> \"0..1\""));
    assert!(!drawing.contains("dashed"));
    assert_eq!(
        retained["diagram"]["edges"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        retained["diagram"]["shapes"].as_array().unwrap().len(),
        3
    );
    Ok(())
}

#[tokio::test]
async fn uml_uses_only_agreed_types_and_endpoint_counts() -> Result<()> {
    let body = diagram(
        r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:Core a owl:Class . ex:Target a owl:Class .
ex:parent a owl:ObjectProperty ; rdfs:domain ex:Core ; rdfs:range ex:Target .
ex:A a sh:NodeShape ; sh:targetClass ex:Core ; sh:property
 [ sh:path ex:name ; sh:datatype xsd:string ; sh:minCount 1 ; sh:maxCount 1 ],
 [ sh:path ex:ambiguous ; sh:datatype xsd:string ],
 [ sh:path ex:parent ; sh:minCount 1 ; sh:maxCount 1 ] .
ex:B a sh:NodeShape ; sh:targetClass ex:Core ; sh:property
 [ sh:path ex:name ; sh:datatype xsd:string ; sh:minCount 1 ; sh:maxCount 1 ],
 [ sh:path ex:ambiguous ; sh:datatype xsd:integer ],
 [ sh:path ex:parent ; sh:minCount 1 ; sh:maxCount 1 ] .
"#,
    )
    .await?;
    let value: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = value["source"].as_str().unwrap();
    assert!(!source.contains("SHACL"));
    assert_eq!(source.matches("{field} ex:name").count(), 1);
    assert!(!source.contains("{field} ex:parent"));
    let name = source
        .lines()
        .find(|line| line.contains("{field} ex:name"))
        .unwrap();
    assert!(name.contains("string [1..1]"));
    let ambiguous = source
        .lines()
        .find(|line| line.contains("{field} ex:ambiguous"))
        .unwrap();
    assert!(!ambiguous.contains(" : "));
    let association = source
        .lines()
        .find(|line| line.contains(" : ") && line.contains("parent"))
        .unwrap();
    assert!(
        association
            .split(" : ")
            .next()
            .unwrap()
            .contains("\"1..1\"")
    );
    assert_eq!(
        value["diagram"]["shapes"].as_array().unwrap().len(),
        2
    );
    Ok(())
}
