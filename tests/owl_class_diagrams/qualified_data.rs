use super::*;
#[derive(serde::Deserialize)]
struct Drawing {
    source:  String,
    diagram: serde_json::Value,
}
async fn drawing(predicate: &str, n: u64, filler: &str) -> Result<Drawing> {
    let body = diagram(&format!(
        "ex:Core a owl:Class; rdfs:subClassOf ex:Parent, [a owl:Restriction; \
         owl:onProperty ex:p; owl:onDataRange {filler}; owl:{predicate} {n}] \
         . ex:Parent a owl:Class . ex:p a owl:DatatypeProperty; rdfs:label \
         \"has time value\" ."
    ))
    .await?;
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
async fn qualified_datatype_counts_are_class_fields_not_object_associations()
-> Result<()> {
    for (predicate, n, expected) in [
        ("qualifiedCardinality", 1, "[1]"),
        ("minQualifiedCardinality", 2, "[2..*]"),
        ("maxQualifiedCardinality", 1, "[0..1]"),
        ("maxQualifiedCardinality", 0, "[0..0]"),
    ] {
        let d = drawing(
            predicate,
            n,
            "<http://www.w3.org/2001/XMLSchema#string>",
        )
        .await?;
        assert!(
            d.source.contains("{field} has time value : xsd:string"),
            "{}",
            d.source
        );
        assert!(d.source.contains(expected));
        assert!(
            !d.source.contains("OWL expression") &&
                !d.source.contains("Literal value")
        );
        assert_eq!(d.diagram["edges"].as_array().unwrap().len(), 5);
        assert!(d.diagram.to_string().contains("onDataRange"));
        assert!(!d.diagram.to_string().contains("#range"));
    }
    Ok(())
}
#[tokio::test]
async fn datatype_zero_minimum_is_omitted_but_complex_ranges_stay_explicit()
-> Result<()> {
    let zero = drawing(
        "minQualifiedCardinality",
        0,
        "<http://www.w3.org/2001/XMLSchema#string>",
    )
    .await?;
    assert!(
        !zero.source.contains("has time value") &&
            !zero.source.contains("OWL expression")
    );
    let complex=drawing("qualifiedCardinality",1,"[owl:unionOf (<http://www.w3.org/2001/XMLSchema#string> <http://www.w3.org/2001/XMLSchema#integer>)]").await?;
    assert!(complex.source.contains("OWL expression"));
    Ok(())
}
