//! One test per OWL restriction to UML notation mapping case.
use super::*;

const PRELUDE: &str = "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix \
                       sh: <http://www.w3.org/ns/shacl#> .
ex:Value a owl:Class ; rdfs:label \"Value\" .
ex:Target a owl:Class ; rdfs:label \"Target\" .
ex:literalValue a owl:DatatypeProperty ; rdfs:label \"literal value\" ;
  rdfs:domain ex:Value ; rdfs:range xsd:string .
ex:name a owl:DatatypeProperty ; rdfs:label \"name\" ; rdfs:domain ex:Core ;
  rdfs:range xsd:string .
ex:data a owl:DatatypeProperty ; rdfs:label \"data\" .
ex:link a owl:ObjectProperty ; rdfs:label \"link\" ; rdfs:range ex:Target .
ex:open a owl:ObjectProperty ; rdfs:label \"open\" .
ex:gold a ex:Target .
";

/// The generated PlantUML source of the documented class `ex:Core`.
async fn source(core: &str) -> Result<String> {
    let body = diagram(&format!(
        "{PRELUDE}ex:Core a owl:Class ; rdfs:label \"Core\" ; {core} ."
    ))
    .await?;
    let json: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    Ok(json["source"].as_str().unwrap().to_owned())
}

/// The source of `ex:Core` with one restriction, asserting it is no box.
async fn restricted(restriction: &str) -> Result<String> {
    let source = source(&format!(
        "rdfs:subClassOf [ a owl:Restriction ; {restriction} ]"
    ))
    .await?;
    assert!(!source.contains("OWL"), "{source}");
    assert!(!source.contains("Literal value"), "{source}");
    Ok(source)
}

fn assert_no_zero_multiplicity(source: &str) {
    for zero in ["[0..0]", "[0]", "\"0..0\"", "\"0\""] {
        assert!(!source.contains(zero), "{zero} in {source}");
    }
}

#[tokio::test]
async fn null_value_max_zero_on_an_inherited_datatype_property_is_a_constraint()
-> Result<()> {
    let source = source(
        "rdfs:subClassOf ex:Value, [ a owl:Restriction ; owl:onProperty \
         ex:literalValue ; owl:maxCardinality 0 ]",
    )
    .await?;
    assert!(
        source.contains("  ..\n  {field} {no literal value}"),
        "{source}"
    );
    assert_no_zero_multiplicity(&source);
    assert!(
        !source.contains("OWL expression") &&
            !source.contains("<<Datatype property>>")
    );
    assert!(!source.contains("Anonymous"), "{source}");
    Ok(())
}

#[tokio::test]
async fn inherited_datatype_count_redefines_the_superclass_attribute()
-> Result<()> {
    let source = source(
        "rdfs:subClassOf ex:Value, [ a owl:Restriction ; owl:onProperty \
         ex:literalValue ; owl:minCardinality 1 ]",
    )
    .await?;
    assert!(
        source
            .contains("{field} literal value : xsd:string [1..*] {redefines}"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn owned_datatype_exact_count_is_one_typed_attribute() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:name ; owl:cardinality 1").await?;
    assert_eq!(
        source.matches("{field} name").count(),
        1,
        "{source}"
    );
    assert!(
        source.contains("{field} name : xsd:string [1] [["),
        "{source}"
    );
    assert!(!source.contains("[1..1]") && !source.contains("redefines"));
    Ok(())
}

#[tokio::test]
async fn datatype_maximum_is_a_bounded_attribute() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:name ; owl:maxCardinality 1").await?;
    assert!(
        source.contains("{field} name : xsd:string [0..1]"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn zero_minimum_imposes_nothing_and_is_omitted() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:data ; owl:minCardinality 0").await?;
    assert!(!source.contains("{field} data"), "{source}");
    Ok(())
}

#[tokio::test]
async fn object_count_is_an_association_to_the_declared_range() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:link ; owl:minCardinality 2").await?;
    assert!(source.contains("\"2..*\" n"), "{source}");
    assert!(source.contains(": link\n"), "{source}");
    assert!(!source.contains("{qualified}"));
    Ok(())
}

#[tokio::test]
async fn object_count_without_a_range_is_an_untyped_attribute() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:open ; owl:cardinality 1").await?;
    assert!(source.contains("{field} open [1] [["), "{source}");
    Ok(())
}

#[tokio::test]
async fn existential_datatype_restriction_is_a_qualified_attribute()
-> Result<()> {
    let source =
        restricted("owl:onProperty ex:data ; owl:someValuesFrom xsd:string")
            .await?;
    assert!(
        source.contains("{field} data : xsd:string [1..*] {qualified}"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn universal_object_restriction_narrows_the_association_target()
-> Result<()> {
    let source =
        restricted("owl:onProperty ex:open ; owl:allValuesFrom ex:Target")
            .await?;
    assert!(source.contains("\"*\" n"), "{source}");
    assert!(source.contains("open\\n{all values}"), "{source}");
    Ok(())
}

#[tokio::test]
async fn universal_datatype_restriction_types_the_attribute() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:data ; owl:allValuesFrom xsd:date")
            .await?;
    assert!(
        source.contains("{field} data : xsd:date {all values}"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn literal_has_value_is_a_fixed_value_attribute() -> Result<()> {
    let source =
        restricted("owl:onProperty ex:data ; owl:hasValue \"fixed\"").await?;
    assert!(
        source.contains("{field} data {&#61; fixed}"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn individual_has_value_is_an_association_to_the_individual() -> Result<()>
{
    let source =
        restricted("owl:onProperty ex:open ; owl:hasValue ex:gold").await?;
    assert!(
        source.contains("open\\n{&#61; ex:gold}"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn qualified_object_zero_is_a_filler_scoped_constraint() -> Result<()> {
    for count in [
        "owl:maxQualifiedCardinality 0",
        "owl:qualifiedCardinality 0",
    ] {
        let source = restricted(&format!(
            "owl:onProperty ex:open ; owl:onClass ex:Target ; {count}"
        ))
        .await?;
        assert!(
            source.contains("{field} {no open in Target}"),
            "{source}"
        );
        assert_no_zero_multiplicity(&source);
    }
    Ok(())
}

#[tokio::test]
async fn qualified_datatype_zero_is_a_filler_scoped_constraint() -> Result<()> {
    let source = restricted(
        "owl:onProperty ex:data ; owl:onDataRange xsd:string ; \
         owl:qualifiedCardinality 0",
    )
    .await?;
    assert!(
        source.contains("{field} {no data in xsd:string}"),
        "{source}"
    );
    assert_no_zero_multiplicity(&source);
    Ok(())
}

#[tokio::test]
async fn neighbour_restrictions_are_members_of_the_neighbour() -> Result<()> {
    let body = diagram(&format!(
        "{PRELUDE}ex:Core a owl:Class . ex:Child a owl:Class ; \
         rdfs:subClassOf ex:Core, [ a owl:Restriction ; owl:onProperty \
         ex:open ; owl:someValuesFrom ex:Target ], [ a owl:Restriction ; \
         owl:onProperty ex:data ; owl:maxCardinality 0 ] ."
    ))
    .await?;
    let json: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = json["source"].as_str().unwrap();
    assert!(
        source.contains("{field} open : Target [1..*] {qualified}"),
        "{source}"
    );
    assert!(source.contains("{field} {no data}"), "{source}");
    assert!(!source.contains("OWL"), "{source}");
    Ok(())
}

#[tokio::test]
async fn boolean_expressions_stay_explicit_and_are_named_by_form() -> Result<()>
{
    let source = source(
        "owl:equivalentClass [ owl:intersectionOf ( ex:Value ex:Target ) ]",
    )
    .await?;
    assert!(
        source.contains("Anonymous OWL\\nintersection 1"),
        "{source}"
    );
    Ok(())
}

#[tokio::test]
async fn shacl_counts_follow_the_same_multiplicity_and_prohibition_rules()
-> Result<()> {
    let body = diagram(&format!(
        "{PRELUDE}ex:Core a owl:Class . ex:S a sh:NodeShape ; sh:targetClass \
         ex:Core ; sh:property [ sh:path ex:name ; sh:datatype xsd:string ; \
         sh:minCount 1 ; sh:maxCount 1 ], [ sh:path ex:data ; sh:datatype \
         xsd:string ; sh:minCount 0 ], [ sh:path ex:open ; sh:maxCount 0 ] ."
    ))
    .await?;
    let json: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = json["source"].as_str().unwrap();
    assert!(
        source.contains("{field} name : string [1] [["),
        "{source}"
    );
    assert!(source.contains("[*] [["), "{source}");
    assert!(source.contains("{field} {no open}"), "{source}");
    assert!(!source.contains("[1..1]") && !source.contains("[0..*]"));
    assert_no_zero_multiplicity(source);
    Ok(())
}
