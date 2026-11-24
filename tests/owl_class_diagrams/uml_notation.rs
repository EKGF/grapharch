//! UML notation for named OWL classes, their relationships and labels.
use super::{support::Drawing, *};

async fn drawing(source: &str) -> Result<Drawing> {
    Drawing::parse(&diagram(source).await?)
}

#[tokio::test]
async fn declared_classes_carry_no_owl_class_stereotype() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf \
         ex:Parent . ex:Parent a owl:Class ; rdfs:label \"Parent\" .",
    )
    .await?;
    assert!(!d.source.contains("owl:Class"), "{}", d.source);
    assert_eq!(
        d.lines(&["class \"Core\" as n", "{"]).len(),
        1,
        "{}",
        d.source
    );
    assert!(!d.lines(&["class \"Core\" as n"])[0].contains("<<"));
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn undeclared_classes_are_external() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf \
         ex:Elsewhere .",
    )
    .await?;
    assert_eq!(
        d.lines(&["class \"ex:", "<<external>>"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn the_unknown_domain_placeholder_is_any() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" . ex:incoming a \
         owl:ObjectProperty ; rdfs:label \"incoming\" ; rdfs:range ex:Core .",
    )
    .await?;
    assert_eq!(
        d.lines(&["&#171;any&#187;", "as unknownDomain"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    assert!(
        !d.description.contains("rdfs:Resource"),
        "{}",
        d.description
    );
    Ok(())
}

#[tokio::test]
async fn rdfs_class_and_rdf_property_types_are_metaclasses() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" . ex:kind a \
         owl:ObjectProperty ; rdfs:label \"kind\" ; rdfs:domain ex:Core ; \
         rdfs:range rdfs:Class . ex:about a owl:ObjectProperty ; rdfs:label \
         \"about\" ; rdfs:domain ex:Core ; rdfs:range \
         <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .",
    )
    .await?;
    for name in ["Class", "Property"] {
        assert_eq!(
            d.lines(&[&format!("class \"{name}\" as n"), "<<metaclass>>"])
                .len(),
            1,
            "{}",
            d.source
        );
    }
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn disjoint_siblings_of_a_shared_parent_form_a_generalization_set()
-> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf \
         ex:Parent ; owl:disjointWith ex:Sibling . ex:Sibling a owl:Class ; \
         rdfs:label \"Sibling\" ; rdfs:subClassOf ex:Parent . ex:Parent a \
         owl:Class ; rdfs:label \"Parent\" .",
    )
    .await?;
    assert_eq!(
        d.lines(&["-|>", ": {disjoint}"]).len(),
        2,
        "{}",
        d.source
    );
    assert!(
        d.lines(&[".. ", "{disjoint}"]).is_empty(),
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn disjoint_classes_without_a_shared_parent_have_a_constraint_line()
-> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; owl:disjointWith \
         ex:Other . ex:Other a owl:Class ; rdfs:label \"Other\" .",
    )
    .await?;
    assert_eq!(
        d.lines(&[" .. ", ": {disjoint}"]).len(),
        1,
        "{}",
        d.source
    );
    assert!(!d.source.contains("disjointWith"));
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn all_disjoint_classes_become_disjoint_constraints() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" . ex:A a owl:Class ; \
         rdfs:label \"A\" . ex:B a owl:Class ; rdfs:label \"B\" . [] a \
         owl:AllDisjointClasses ; owl:members ( ex:Core ex:A ex:B ) .",
    )
    .await?;
    assert_eq!(
        d.lines(&[" .. ", ": {disjoint}"]).len(),
        2,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn named_equivalence_is_a_dashed_equivalent_line_never_a_triangle()
-> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; owl:equivalentClass \
         ex:Same . ex:Same a owl:Class ; rdfs:label \"Same\" .",
    )
    .await?;
    assert_eq!(
        d.lines(&[" .. ", "&#171;equivalent&#187;"]).len(),
        1,
        "{}",
        d.source
    );
    assert!(!d.source.contains("|>"), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn colliding_class_names_add_a_from_line_only_on_those_boxes()
-> Result<()> {
    let body = support::diagram_with_prefixes(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:isDefinedBy \
         ex:onto ; rdfs:subClassOf ex:Capability, \
         <https://example.org/two/Capability>, ex:Area . ex:Capability a \
         owl:Class ; rdfs:label \"Capability\" ; rdfs:isDefinedBy ex:onto \
         . <https://example.org/two/Capability> a owl:Class ; rdfs:label \
         \"Capability\" ; rdfs:isDefinedBy ex:onto . ex:Area a owl:Class ; \
         rdfs:label \"Area\" ; rdfs:isDefinedBy ex:onto .",
        &[
            ("https://example.org/", "ex"),
            ("https://example.org/two/", "two"),
        ],
        "Core",
    )
    .await?;
    let d = Drawing::parse(&body)?;
    assert_eq!(
        d.lines(&["class \"Capability\\n&#40;from ex&#41;\" as n"])
            .len(),
        1,
        "{}",
        d.source
    );
    assert_eq!(
        d.lines(&["class \"Capability\\n&#40;from two&#41;\" as n"])
            .len(),
        1,
        "{}",
        d.source
    );
    assert_eq!(
        d.lines(&["class \"Area\" as n"]).len(),
        1,
        "{}",
        d.source
    );
    assert!(
        !d.source.contains("two:Capability"),
        "{}",
        d.source
    );
    Ok(())
}

#[tokio::test]
async fn inverse_properties_are_one_association_with_both_end_names()
-> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" . ex:Target a owl:Class ; \
         rdfs:label \"Target\" . ex:has a owl:ObjectProperty ; rdfs:label \
         \"has\" ; rdfs:domain ex:Core ; rdfs:range ex:Target ; owl:inverseOf \
         ex:of . ex:of a owl:ObjectProperty ; rdfs:label \"of\" ; rdfs:domain \
         ex:Target ; rdfs:range ex:Core .",
    )
    .await?;
    let ends = d.lines(&["\"of\" -- \"has\""]);
    assert_eq!(ends.len(), 1, "{}", d.source);
    assert!(d.lines(&[": has"]).is_empty(), "{}", d.source);
    assert!(d.lines(&[": of"]).is_empty(), "{}", d.source);
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn complement_is_a_not_constraint_note() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf [ a \
         owl:Class ; owl:complementOf ex:Other ] . ex:Other a owl:Class ; \
         rdfs:label \"Other\" .",
    )
    .await?;
    assert_eq!(
        d.lines(&["note \"&#123;not Other&#125;\" as N"]).len(),
        1,
        "{}",
        d.source
    );
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn one_of_is_an_enumeration_with_its_literals() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; owl:equivalentClass [ a \
         owl:Class ; owl:oneOf ( ex:red ex:green ) ] . ex:red rdfs:label \
         \"red\" . ex:green rdfs:label \"green\" .",
    )
    .await?;
    assert_eq!(
        d.lines(&["class \"Core\" as n", "<<enumeration>>"]).len(),
        1,
        "{}",
        d.source
    );
    assert!(d.diagram["axioms"][0]["expression"]["one_of"].is_array());
    for literal in ["  {field} red", "  {field} green"] {
        assert_eq!(d.lines(&[literal]).len(), 1, "{}", d.source);
    }
    d.assert_uml_only();
    Ok(())
}

#[tokio::test]
async fn the_description_verbalizes_the_uml_projection() -> Result<()> {
    let d = drawing(
        "ex:Core a owl:Class ; rdfs:label \"Core\" ; rdfs:subClassOf \
         ex:Parent ; owl:disjointWith ex:Other . ex:Parent a owl:Class ; \
         rdfs:label \"Parent\" . ex:Other a owl:Class ; rdfs:label \"Other\" \
         . ex:Target a owl:Class ; rdfs:label \"Target\" . ex:has a \
         owl:ObjectProperty ; rdfs:label \"has\" ; rdfs:domain ex:Core ; \
         rdfs:range ex:Target .",
    )
    .await?;
    for sentence in [
        "Core is a kind of Parent.",
        "Core and Other are disjoint.",
        "Core is associated with Target by has.",
    ] {
        assert!(
            d.description.contains(sentence),
            "{}",
            d.description
        );
    }
    for raw in ["rdfs:", "owl:", "Diagram labels"] {
        assert!(!d.description.contains(raw), "{}", d.description);
    }
    Ok(())
}
