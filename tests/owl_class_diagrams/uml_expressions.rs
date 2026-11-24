//! UML notation for anonymous OWL class expressions: equivalence, nesting,
//! boolean fillers and union domains.
use super::{support::Drawing, *};

const PRELUDE: &str = "ex:Core a owl:Class ; rdfs:label \"Core\" .
ex:Value a owl:Class ; rdfs:label \"Value\" .
ex:Target a owl:Class ; rdfs:label \"Target\" .
ex:Other a owl:Class ; rdfs:label \"Other\" .
ex:p a owl:ObjectProperty ; rdfs:label \"p\" .
ex:q a owl:ObjectProperty ; rdfs:label \"q\" .
";

async fn drawing(axioms: &str) -> Result<Drawing> {
    Drawing::parse(&diagram(&format!("{PRELUDE}ex:Core {axioms} .")).await?)
}

#[tokio::test]
async fn equivalence_to_a_restriction_is_a_defining_association() -> Result<()>
{
    let d = drawing(
        "owl:equivalentClass [ a owl:Restriction ; owl:onProperty ex:p ; \
         owl:someValuesFrom ex:Target ]",
    )
    .await?;
    assert_eq!(
        d.lines(&["\"1..*\" n", ": p\\n{qualified, defining}"])
            .len(),
        1,
        "{}",
        d.source
    );
    assert!(!d.source.contains("|>"), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn equivalence_to_an_intersection_generalizes_its_named_part()
-> Result<()> {
    let d = drawing(
        "owl:equivalentClass [ a owl:Class ; owl:intersectionOf ( ex:Value [ \
         a owl:Restriction ; owl:onProperty ex:p ; owl:someValuesFrom \
         ex:Target ] ) ]",
    )
    .await?;
    assert_eq!(d.lines(&["-|>"]).len(), 1, "{}", d.source);
    assert_eq!(
        d.lines(&[": p\\n{qualified, defining}"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn intersection_superclass_is_multiple_generalization() -> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Class ; owl:intersectionOf ( ex:Value \
         ex:Target ) ]",
    )
    .await?;
    assert_eq!(d.lines(&["-|>"]).len(), 2, "{}", d.source);
    assert!(d.lines(&["defining"]).is_empty(), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn an_intersection_of_restrictions_draws_each_restriction() -> Result<()>
{
    let d = drawing(
        "rdfs:subClassOf [ a owl:Class ; owl:intersectionOf ( [ a \
         owl:Restriction ; owl:onProperty ex:p ; owl:someValuesFrom ex:Target \
         ] [ a owl:Restriction ; owl:onProperty ex:q ; owl:someValuesFrom \
         ex:Value ] ) ]",
    )
    .await?;
    assert_eq!(
        d.lines(&[": p\\n{qualified}"]).len(),
        1,
        "{}",
        d.source
    );
    assert_eq!(
        d.lines(&[": q\\n{qualified}"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn nested_existentials_are_a_path_member() -> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; \
         owl:someValuesFrom [ a owl:Restriction ; owl:onProperty ex:q ; \
         owl:someValuesFrom ex:Target ] ]",
    )
    .await?;
    assert_eq!(
        d.lines(&["  {field} p.q : Target [1..*]"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn other_nested_restrictions_are_an_ocl_constraint_note() -> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; \
         owl:someValuesFrom [ a owl:Restriction ; owl:onProperty ex:q ; \
         owl:allValuesFrom ex:Target ] ]",
    )
    .await?;
    let notes = d.lines(&["note \"", "as N"]);
    assert_eq!(notes.len(), 1, "{}", d.source);
    for text in ["p-&#62;exists", "q-&#62;forAll", "oclIsKindOf&#40;Target"] {
        assert!(notes[0].contains(text), "{text}: {}", d.source);
    }
    assert_eq!(d.lines(&["N0 .. n"]).len(), 1, "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn a_union_filler_has_one_association_end_per_member_and_xor()
-> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; \
         owl:someValuesFrom [ a owl:Class ; owl:unionOf ( ex:Target ex:Value \
         ) ] ]",
    )
    .await?;
    assert_eq!(
        d.lines(&["\"1..*\" n", ": p\\n{qualified, xor}"]).len(),
        2,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn an_intersection_filler_is_a_constraint_note() -> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ex:p ; \
         owl:someValuesFrom [ a owl:Class ; owl:intersectionOf ( ex:Target \
         ex:Value ) ] ]",
    )
    .await?;
    let notes = d.lines(&["note \"", "as N"]);
    assert_eq!(notes.len(), 1, "{}", d.source);
    assert!(
        notes[0].contains("and x.oclIsKindOf"),
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn equivalence_to_a_union_is_a_complete_generalization_set() -> Result<()>
{
    let d = drawing(
        "owl:equivalentClass [ a owl:Class ; owl:unionOf ( ex:Target ex:Value \
         ) ]",
    )
    .await?;
    assert_eq!(
        d.lines(&["-|>", ": {complete}"]).len(),
        2,
        "{}",
        d.source
    );
    assert!(!d.source.contains(" or "), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn a_union_domain_is_one_xor_association_per_member() -> Result<()> {
    let d = Drawing::parse(
        &diagram(
            "ex:Core a owl:Class ; rdfs:label \"Core\" . ex:A a owl:Class ; \
             rdfs:label \"A\" . ex:B a owl:Class ; rdfs:label \"B\" . \
             ex:inCore a owl:ObjectProperty ; rdfs:label \"in core\" ; \
             rdfs:domain [ a owl:Class ; owl:unionOf ( ex:A ex:B ) ] ; \
             rdfs:range ex:Core .",
        )
        .await?,
    )?;
    assert_eq!(
        d.lines(&[": in core\\n{xor}"]).len(),
        2,
        "{}",
        d.source
    );
    assert!(!d.source.contains(" or "), "{}", d.source);
    assert!(!d.source.contains("rectangle"), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn undeclared_property_and_filler_are_an_external_association()
-> Result<()> {
    let d = Drawing::parse(
        &diagram(
            "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf [ a \
             owl:Restriction ; owl:onProperty ex:hasPart ; owl:someValuesFrom \
             ex:Court ] .",
        )
        .await?,
    )?;
    assert_eq!(
        d.lines(&["<<external>>"]).len(),
        1,
        "{}",
        d.source
    );
    assert_eq!(
        d.lines(&["\"1..*\" n", ": ex:\\nhasPart\\n{qualified}"])
            .len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn a_restriction_on_an_inverse_property_points_from_the_filler()
-> Result<()> {
    let d = drawing(
        "rdfs:subClassOf [ a owl:Restriction ; owl:onProperty [ owl:inverseOf \
         ex:p ] ; owl:someValuesFrom ex:Target ]",
    )
    .await?;
    let target = d
        .source
        .lines()
        .find(|l| l.starts_with("class \"Target\" as n"))
        .and_then(|l| l.split(" as ").nth(1))
        .and_then(|l| l.split(' ').next())
        .expect("a Target box")
        .to_owned();
    assert_eq!(
        d.lines(&[&format!("{target} \"1..*\" -")]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}
