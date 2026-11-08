use super::*;
#[derive(serde::Deserialize)]
struct Drawing {
    source:  String,
    svg:     String,
    diagram: serde_json::Value,
}
async fn drawing(constraint: &str) -> Result<Drawing> {
    let body = diagram(&format!(
        "ex:Core a owl:Class; rdfs:subClassOf ex:Parent, [a owl:Restriction; \
         {constraint}] . ex:Parent a owl:Class . ex:Target a owl:Class . ex:p \
         a owl:ObjectProperty ."
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
async fn qualified_maximum_including_zero_is_a_target_end_prohibition_or_bound()
-> Result<()> {
    for n in [0, 1, 3] {
        let d = drawing(&format!(
            "owl:onProperty ex:p; owl:onClass ex:Target; \
             owl:maxQualifiedCardinality {n}"
        ))
        .await?;
        assert!(
            d.source.contains(&format!("\"0..{n}\" n")),
            "{}",
            d.source
        );
        assert!(
            d.source.contains("{qualified}") &&
                !d.source.contains("OWL expression")
        );
        assert!(d.svg.contains(&format!("0..{n}")));
        assert_eq!(d.diagram["edges"].as_array().unwrap().len(), 5);
        assert!(d.diagram.to_string().contains("maxQualifiedCardinality"));
        assert!(!d.diagram.to_string().contains("#range"));
    }
    Ok(())
}
#[tokio::test]
async fn maximum_does_not_guarantee_outer_existence() -> Result<()> {
    let body = diagram(
        "ex:Core a owl:Class; rdfs:subClassOf [a owl:Restriction; \
         owl:onProperty ex:p; owl:onClass ex:Target; \
         owl:maxQualifiedCardinality 1], [a owl:Restriction; owl:onProperty \
         ex:p; owl:someValuesFrom [a owl:Restriction; owl:onProperty ex:q; \
         owl:onClass ex:Target; owl:minQualifiedCardinality 0]] . ex:Target a \
         owl:Class . ex:p a owl:ObjectProperty . ex:q a owl:ObjectProperty .",
    )
    .await?;
    let d: Drawing = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    assert!(d.source.contains("\"0..1\" n"));
    assert!(
        d.source.contains("[1..*]"),
        "upper bound cannot replace existence"
    );
    Ok(())
}
#[tokio::test]
async fn maximum_keeps_complex_or_malformed_restrictions_explicit() -> Result<()>
{
    for clause in [
        "owl:onProperty [owl:inverseOf ex:p]; owl:onClass ex:Target; \
         owl:maxQualifiedCardinality 1",
        "owl:onProperty ex:p; owl:onClass [owl:unionOf (ex:Target \
         ex:Parent)]; owl:maxQualifiedCardinality 1",
        "owl:onProperty ex:p; owl:onClass ex:Target; \
         owl:maxQualifiedCardinality -1",
        "owl:onProperty ex:p; owl:onClass ex:Target; \
         owl:maxQualifiedCardinality \"1\"",
        "owl:onProperty ex:p; owl:onClass ex:Target; \
         owl:maxQualifiedCardinality 1; owl:someValuesFrom ex:Parent",
    ] {
        let d = drawing(clause).await?;
        assert!(d.source.contains("OWL expression"));
        assert!(!d.source.contains("{qualified}"));
    }
    Ok(())
}
