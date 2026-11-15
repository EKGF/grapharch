use super::*;

async fn labels(source: &str) -> Result<(String, String)> {
    let body = diagram(source).await?;
    let projection: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    Ok((
        projection["diagram"].to_string(),
        projection["source"].as_str().unwrap().to_owned(),
    ))
}

#[tokio::test]
async fn neighbours_from_another_ontology_show_their_prefixed_name()
-> Result<()> {
    let (diagram, source) = labels(
        r#"
ex:Core a owl:Class; rdfs:label "Core"; rdfs:isDefinedBy <https://example.org/a>;
  rdfs:subClassOf ex:Authority .
ex:Authority a owl:Class; rdfs:label "Authority"; rdfs:isDefinedBy <https://example.org/b> .
ex:governs a owl:ObjectProperty; rdfs:label "governs"; rdfs:isDefinedBy <https://example.org/b>;
  rdfs:domain ex:Core; rdfs:range ex:Authority .
ex:note a owl:DatatypeProperty; rdfs:label "note"; rdfs:isDefinedBy <https://example.org/b>;
  rdfs:domain ex:Core .
"#,
    )
    .await?;
    for expected in [
        "Authority (ex:Authority)",
        "governs (ex:governs)",
        "note (ex:note)",
    ] {
        assert!(
            diagram.contains(expected),
            "{expected}: {diagram}"
        );
    }
    assert!(
        source.contains("Authority &#40;ex:Authority&#41;"),
        "{source}"
    );
    assert!(diagram.contains(r#""label":"Core""#), "{diagram}");
    Ok(())
}

#[tokio::test]
async fn neighbours_from_the_same_ontology_keep_their_label() -> Result<()> {
    let (diagram, _) = labels(
        r#"
ex:Core a owl:Class; rdfs:label "Core"; rdfs:isDefinedBy <https://example.org/a>;
  rdfs:subClassOf ex:Family .
ex:Family a owl:Class; rdfs:label "Family"; rdfs:isDefinedBy <https://example.org/a> .
ex:holds a owl:ObjectProperty; rdfs:label "holds"; rdfs:domain ex:Core; rdfs:range ex:Family .
"#,
    )
    .await?;
    for unexpected in ["(ex:Family)", "(ex:holds)", "(ex:Core)"] {
        assert!(
            !diagram.contains(unexpected),
            "{unexpected}: {diagram}"
        );
    }
    assert!(
        diagram.contains(r#""label":"Family""#),
        "{diagram}"
    );
    Ok(())
}

#[tokio::test]
async fn a_local_class_and_property_sharing_a_label_keep_their_labels()
-> Result<()> {
    let (diagram, _) = labels(
        r#"
ex:Core a owl:Class; rdfs:label "Publication profile" .
ex:Run a owl:Class; rdfs:label "Publication run" .
ex:publicationProfile a owl:ObjectProperty; rdfs:label "Publication profile";
  rdfs:domain ex:Run; rdfs:range ex:Core .
ex:publicationRun a owl:ObjectProperty; rdfs:label "Publication run";
  rdfs:domain ex:Core; rdfs:range ex:Run .
"#,
    )
    .await?;
    assert!(!diagram.contains("(ex:"), "{diagram}");
    Ok(())
}
