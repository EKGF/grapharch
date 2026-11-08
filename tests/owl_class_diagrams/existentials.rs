use super::*;

#[derive(serde::Deserialize)]
struct Drawing {
    source:  String,
    svg:     String,
    diagram: serde_json::Value,
}

async fn drawing(constraint: &str) -> Result<Drawing> {
    let body = diagram(&format!(
        "ex:Core a owl:Class ; rdfs:subClassOf ex:Document, [ a \
         owl:Restriction ; {constraint} ] .\nex:Document a owl:Class . \
         ex:Other a owl:Class . ex:isMemberOf a owl:ObjectProperty ."
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
async fn simple_existential_is_a_scoped_association_without_global_multiplicity()
-> Result<()> {
    let drawing = drawing(
        "owl:onProperty ex:isMemberOf ; owl:someValuesFrom ex:Document",
    )
    .await?;
    assert!(
        drawing.source.contains("{at least one ex:Document}"),
        "{}",
        drawing.source
    );
    assert!(drawing.source.contains("-up-|>"));
    assert!(drawing.source.contains("-up->"));
    assert!(
        !drawing.source.contains("someValuesFrom") &&
            !drawing.source.contains("onProperty")
    );
    assert!(
        !drawing.source.contains("OWL expression") &&
            !drawing.source.contains("1..*")
    );
    assert!(drawing.svg.contains("at least one"));
    assert_eq!(
        drawing.diagram["edges"].as_array().unwrap().len(),
        4
    );
    assert!(drawing.diagram.to_string().contains("someValuesFrom"));
    let other = diagram_with_focus(
        "ex:Core a owl:Class ; rdfs:subClassOf [ a owl:Restriction ; \
         owl:onProperty ex:p ; owl:someValuesFrom ex:Document ] . ex:Document \
         a owl:Class . ex:p a owl:ObjectProperty . ex:Other a owl:Class ; \
         rdfs:subClassOf ex:Core .",
        "ex",
        "Other",
    )
    .await?;
    let other: Drawing = serde_json::from_str(
        other
            .split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    assert!(
        !other.source.contains("at least one"),
        "no global or inherited constraint is invented"
    );
    Ok(())
}

#[tokio::test]
async fn unsupported_restrictions_are_not_simplified_to_existentials()
-> Result<()> {
    for constraint in [
        "owl:onProperty ex:unknown ; owl:someValuesFrom ex:Document",
        "owl:onProperty ex:isMemberOf ; owl:allValuesFrom ex:Document",
        "owl:onProperty [ owl:inverseOf ex:isMemberOf ] ; owl:someValuesFrom \
         ex:Document",
        "owl:onProperty ex:isMemberOf ; owl:someValuesFrom [ owl:unionOf \
         (ex:Document ex:Other) ]",
        "owl:onProperty ex:isMemberOf ; owl:someValuesFrom ex:Document ; \
         owl:allValuesFrom ex:Other",
        "owl:onProperty ex:isMemberOf ; owl:someValuesFrom ex:Document ; \
         owl:unknownConstraint ex:Other",
    ] {
        let drawing = drawing(constraint).await?;
        assert!(
            !drawing.source.contains("at least one"),
            "{constraint}\n{}",
            drawing.source
        );
        assert!(drawing.source.contains("OWL expression"));
    }
    Ok(())
}
