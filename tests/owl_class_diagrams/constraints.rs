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
    assert!(drawing.contains("SHACL S1"), "{drawing}");
    assert!(drawing.contains("SHACL S2"));
    assert!(drawing.contains("SHACL S3"));
    assert!(drawing.contains("ex:name") && drawing.contains("string"));
    assert!(drawing.contains("1..3") && drawing.contains("0..0"));
    assert!(!drawing.contains("9.."));
    assert!(drawing.contains("-[#3B6E8F,dashed]right->"));
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
