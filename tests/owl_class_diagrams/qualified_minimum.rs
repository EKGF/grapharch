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
         {constraint}] . ex:Parent a owl:Class . ex:Minister a owl:Class . \
         ex:managedBy a owl:ObjectProperty ."
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
async fn zero_qualified_minimum_has_no_visual_restriction_but_retains_facts()
-> Result<()> {
    let d = drawing(
        "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
         owl:minQualifiedCardinality \"0\"^^<http://www.w3.org/2001/XMLSchema#nonNegativeInteger>",
    )
    .await?;
    assert!(
        !d.source.contains("OWL expression"),
        "{}",
        d.source
    );
    assert!(!d.source.contains("Minister") && !d.source.contains("managedBy"));
    assert!(d.source.contains("-up-|>"));
    assert_eq!(d.diagram["edges"].as_array().unwrap().len(), 5);
    assert!(d.diagram.to_string().contains("minQualifiedCardinality"));
    Ok(())
}
#[tokio::test]
async fn positive_qualified_minimum_is_at_the_named_target_end() -> Result<()> {
    for n in [1, 3] {
        let d = drawing(&format!(
            "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
             owl:minQualifiedCardinality {n}"
        ))
        .await?;
        assert!(
            d.source.contains(&format!("\"{n}..*\" n")),
            "{}",
            d.source
        );
        assert!(d.source.contains("{qualified}"));
        assert!(
            !d.source.contains("OWL expression") &&
                !d.source.contains("minQualifiedCardinality")
        );
        assert!(d.svg.contains(&format!("{n}..*")));
        assert_eq!(d.diagram["edges"].as_array().unwrap().len(), 5);
        assert!(!d.diagram.to_string().contains("#range"));
    }
    Ok(())
}
#[tokio::test]
async fn qualified_minimum_does_not_simplify_other_or_ambiguous_constraints()
-> Result<()> {
    for constraint in [
        "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
         owl:qualifiedCardinality 0",
        "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
         owl:minQualifiedCardinality -1",
        "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
         owl:minQualifiedCardinality \"1\"",
        "owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
         owl:minQualifiedCardinality 0; owl:allValuesFrom ex:Parent",
        "owl:onProperty [owl:inverseOf ex:managedBy]; owl:onClass \
         ex:Minister; owl:minQualifiedCardinality 1",
        "owl:onProperty ex:managedBy; owl:onClass [owl:unionOf (ex:Minister \
         ex:Parent)]; owl:minQualifiedCardinality 1",
    ] {
        let d = drawing(constraint).await?;
        assert!(
            d.source.contains("OWL expression"),
            "{constraint}\n{}",
            d.source
        );
        assert!(!d.source.contains("{qualified}"));
    }
    Ok(())
}

#[tokio::test]
async fn qualified_minimum_is_not_inherited_or_removed_inside_a_boolean_expression()
-> Result<()> {
    for (source, focus) in [
        (
            "ex:Core a owl:Class; rdfs:subClassOf [a owl:Restriction; \
             owl:onProperty ex:managedBy; owl:onClass ex:Minister; \
             owl:minQualifiedCardinality 1] . ex:Other a owl:Class; \
             rdfs:subClassOf ex:Core . ex:Minister a owl:Class . ex:managedBy \
             a owl:ObjectProperty .",
            "Other",
        ),
        (
            "ex:Core a owl:Class; rdfs:subClassOf [owl:unionOf (ex:Parent [a \
             owl:Restriction; owl:onProperty ex:managedBy; owl:onClass \
             ex:Minister; owl:minQualifiedCardinality 0])] . ex:Parent a \
             owl:Class . ex:Minister a owl:Class . ex:managedBy a \
             owl:ObjectProperty .",
            "Core",
        ),
    ] {
        let body = diagram_with_focus(source, "ex", focus).await?;
        let d: Drawing = serde_json::from_str(
            body.split("```grapharch-class-diagram\n")
                .nth(1)
                .unwrap()
                .split("\n```")
                .next()
                .unwrap(),
        )?;
        assert!(!d.source.contains("{qualified}"));
        if focus == "Core" {
            assert!(d.source.contains("OWL expression"));
        }
    }
    Ok(())
}
