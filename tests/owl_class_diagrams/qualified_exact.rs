use super::*;
#[derive(serde::Deserialize)]
struct Drawing {
    source:  String,
    svg:     String,
    diagram: serde_json::Value,
}
async fn drawing(clauses: &str) -> Result<Drawing> {
    let body = diagram(&format!(
        "ex:Core a owl:Class; rdfs:subClassOf ex:Parent, {clauses} . \
         ex:Parent a owl:Class . ex:Person a owl:Class . ex:Body a owl:Class \
         . ex:plays a owl:ObjectProperty . ex:member a owl:ObjectProperty . \
         ex:Child a owl:Class; rdfs:subClassOf ex:Core ."
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
async fn positive_exact_count_is_qualified_and_scoped_to_named_target()
-> Result<()> {
    for count in [1, 2] {
        let d = drawing(&format!(
            "[a owl:Restriction; owl:onProperty ex:plays; owl:onClass \
             ex:Person; owl:qualifiedCardinality {count}]"
        ))
        .await?;
        assert!(
            d.source.contains(&format!("\"{count}\" n")),
            "{}",
            d.source
        );
        assert!(d.source.contains("{qualified}"));
        assert!(!d.source.contains("OWL expression"));
        assert!(d.source.contains("-up-|>"));
        assert!(d.svg.contains("qualified"));
        assert!(d.diagram.to_string().contains("qualifiedCardinality"));
        assert!(!d.diagram.to_string().contains("FunctionalProperty"));
    }
    Ok(())
}
#[tokio::test]
async fn nested_zero_preserves_outer_existence_without_expression_boxes()
-> Result<()> {
    let d = drawing(
        "[a owl:Restriction; owl:onProperty ex:plays; owl:someValuesFrom [a \
         owl:Restriction; owl:onProperty ex:member; owl:onClass ex:Body; \
         owl:minQualifiedCardinality 0]]",
    )
    .await?;
    assert!(d.source.contains("[1..*]"), "{}", d.source);
    assert!(
        !d.source.contains("OWL expression") &&
            !d.source.contains("Literal value")
    );
    assert!(
        d.diagram.to_string().contains("minQualifiedCardinality"),
        "nested proof facts retained"
    );
    assert!(d.diagram.to_string().contains("someValuesFrom"));
    Ok(())
}
#[tokio::test]
async fn same_property_positive_qualified_count_covers_nested_top_existence()
-> Result<()> {
    let nested = "[a owl:Restriction; owl:onProperty ex:plays; \
                  owl:someValuesFrom [a owl:Restriction; owl:onProperty \
                  ex:member; owl:onClass ex:Body; owl:minQualifiedCardinality \
                  0]]";
    let exact = "[a owl:Restriction; owl:onProperty ex:plays; owl:onClass \
                 ex:Person; owl:qualifiedCardinality 1]";
    let d = drawing(&format!("{nested}, {exact}")).await?;
    assert!(
        !d.source.contains("OWL expression"),
        "{}",
        d.source
    );
    assert!(
        !d.source.contains("[1..*]"),
        "positive same-property count entails existence"
    );
    assert!(d.source.contains("\"1\" n"));
    assert_eq!(d.diagram["edges"].as_array().unwrap().len(), 12);
    let other = drawing(&format!(
        "{nested}, {}",
        exact.replace("ex:plays", "ex:member")
    ))
    .await?;
    assert!(
        other.source.contains("[1..*]"),
        "different-property count cannot cover existence"
    );
    Ok(())
}
#[tokio::test]
async fn nested_nonzero_or_extra_constraints_are_not_treated_as_top()
-> Result<()> {
    for inner in [
        "owl:minQualifiedCardinality 1",
        "owl:qualifiedCardinality 0",
        "owl:maxQualifiedCardinality 0",
        "owl:minQualifiedCardinality 0; owl:allValuesFrom ex:Person",
        "owl:minQualifiedCardinality \"0\"",
    ] {
        let d = drawing(&format!(
            "[a owl:Restriction; owl:onProperty ex:plays; owl:someValuesFrom \
             [a owl:Restriction; owl:onProperty ex:member; owl:onClass \
             ex:Body; {inner}]]"
        ))
        .await?;
        assert!(d.source.contains("OWL expression"));
        assert!(!d.source.contains("[1..*]"));
    }
    Ok(())
}
