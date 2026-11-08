use super::*;

#[tokio::test]
async fn range_only_association_uses_a_non_asserted_dotted_domain_placeholder()
-> Result<()> {
    let body = diagram(
        "ex:Core a owl:Class . ex:p a owl:ObjectProperty; rdfs:range ex:Core .",
    )
    .await?;
    let projection: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    let source = projection["source"].as_str().unwrap();
    assert!(source.contains("rdfs:Resource"), "{source}");
    assert!(
        source.contains("line.dotted"),
        "placeholder must be visually distinct: {source}"
    );
    assert!(
        !source.contains("rdfs:range") && !source.contains("Object property"),
        "range is shown as an association, not an RDF property box"
    );
    let edges = projection["diagram"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(
        edges[0]["predicate"],
        "http://www.w3.org/2000/01/rdf-schema#range"
    );
    assert!(
        !projection["diagram"]
            .to_string()
            .contains("http://www.w3.org/2000/01/rdf-schema#domain"),
        "placeholder must not invent a domain assertion"
    );
    assert!(
        projection["svg"]
            .as_str()
            .unwrap()
            .contains("stroke-dasharray")
    );
    Ok(())
}

#[tokio::test]
async fn declared_or_ambiguous_domains_and_ranges_do_not_use_a_placeholder()
-> Result<()> {
    for assertions in [
        "ex:p a owl:ObjectProperty; rdfs:domain ex:Other; rdfs:range ex:Core .",
        "ex:p a owl:ObjectProperty; rdfs:domain ex:Other, ex:Third; \
         rdfs:range ex:Core .",
        "ex:p a owl:ObjectProperty; rdfs:range ex:Core, ex:Other .",
        "ex:p a owl:DatatypeProperty; rdfs:range ex:Core .",
    ] {
        let body = diagram(&format!(
            "ex:Core a owl:Class . ex:Other a owl:Class . ex:Third a \
             owl:Class . {assertions}"
        ))
        .await?;
        let projection: serde_json::Value = serde_json::from_str(
            body.split("```grapharch-class-diagram\n")
                .nth(1)
                .unwrap()
                .split("\n```")
                .next()
                .unwrap(),
        )?;
        assert!(
            !projection["source"]
                .as_str()
                .unwrap()
                .contains("line.dotted"),
            "{assertions}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn authored_labels_enrich_all_uml_names_without_losing_identity()
-> Result<()> {
    let body = diagram(r#"
ex:Core a owl:Class; rdfs:label "Context"@en, "Contexte"@fr; rdfs:subClassOf ex:Other .
ex:Other a owl:Class; rdfs:label "Context"@en .
ex:time a owl:DatatypeProperty; rdfs:label "recorded at"; rdfs:domain ex:Core; rdfs:range <http://www.w3.org/2001/XMLSchema#dateTime> .
ex:p a owl:ObjectProperty; rdfs:label "delegates control to"@en; rdfs:range ex:Core .
"#).await?;
    let projection: serde_json::Value = serde_json::from_str(
        body.split("```grapharch-class-diagram\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap(),
    )?;
    assert_eq!(
        projection["diagram"]["focus"]["label"],
        "Context (ex:Core)"
    );
    let source = projection["source"].as_str().unwrap();
    for expected in ["delegates control to", "recorded at", "Context"] {
        assert!(source.contains(expected), "{source}");
    }
    assert!(!source.contains("Contexte"));
    assert!(
        projection["diagram"]
            .to_string()
            .contains("https://example.org/p")
    );
    assert!(
        projection["diagram"]
            .to_string()
            .contains("Context (ex:Other)")
    );
    Ok(())
}
