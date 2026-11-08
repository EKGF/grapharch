use super::*;

#[tokio::test]
async fn ontology_abstract_and_other_authored_introduction_survive_source_removal()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/ontology.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix skos:<http://www.w3.org/2004/02/skos/core#> .
<https://example.org/vocab/> a owl:Ontology ; rdfs:label "Authored introduction" ;
 dct:abstract "Original abstract first."@en, "Ne pas publier."@fr ;
 dct:description "Additional authored detail."@en ;
 skos:definition "Distinct formal definition."@en ; rdfs:comment "Distinct source comment."@en, "Additional authored detail."@en .
<https://example.org/vocab/Thing> a owl:Class ; rdfs:label "Thing" ; rdfs:comment "A thing." .
"#).await?;
    let built = collect(source.path()).await?;
    let mut rows = built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body WHERE { GRAPH ?g { ?fragment ga:subject <https://example.org/vocab/> ; ga:body ?body } }".into()).await?;
    let mut found = false;
    while let Some(row) = rows.next().await? {
        let oxrdf::Term::Literal(body) = &row["body"] else {
            continue;
        };
        if !body.value().contains("## Overview") {
            continue;
        }
        found = true;
        let mut previous = 0;
        for text in [
            "Original abstract first.",
            "Additional authored detail.",
            "Distinct formal definition.",
            "Distinct source comment.",
        ] {
            assert_eq!(
                body.value().matches(text).count(),
                1,
                "missing or duplicate authored paragraph: {text}"
            );
            let position = body.value().find(text).unwrap();
            assert!(position >= previous);
            previous = position;
        }
        assert!(!body.value().contains("Ne pas publier"));
    }
    assert!(found);
    let output = tempfile::tempdir()?;
    let archive = output.path().join("retained.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required");
    };
    for file in ["index.html", "main.typ"] {
        let body =
            tokio::fs::read_to_string(publication.directory.join(file)).await?;
        for text in [
            "Original abstract first.",
            "Additional authored detail.",
            "Distinct formal definition.",
            "Distinct source comment.",
        ] {
            assert!(body.contains(text), "{file} missing {text}");
        }
    }
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn diagram_endpoint_types_use_other_pinned_family_members() -> Result<()>
{
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), format!("{DEFINITIONS}\n<https://example.org/relations/> <https://ekgf.org/ontology/artifact-governance#inFamily> <urn:test:family> ; <http://www.w3.org/ns/shacl#declare> [ <http://www.w3.org/ns/shacl#prefix> \"rel\" ; <http://www.w3.org/ns/shacl#namespace> \"https://example.org/relations/\" ] .")).await?;
    tokio::fs::write(source.path().join("sources/classes.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/vocab/> a owl:Ontology; rdfs:label "Classes" .
<https://example.org/vocab/Part> a owl:Class; rdfs:label "Part"; rdfs:subClassOf [a owl:Restriction; owl:onProperty <https://example.org/relations/member>; owl:someValuesFrom <https://example.org/vocab/Whole>] .
<https://example.org/vocab/Whole> a owl:Class; rdfs:label "Whole" .
"#).await?;
    tokio::fs::write(source.path().join("sources/relations.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/relations/> a owl:Ontology; rdfs:label "Relations" .
<https://example.org/relations/member> a owl:ObjectProperty; rdfs:label "member" .
"#).await?;
    let built = collect(source.path()).await?;
    let mut rows = built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body WHERE { GRAPH ?g { ?fragment ga:subject <https://example.org/vocab/Part>; ga:body ?body } }".into()).await?;
    let mut qualified = false;
    while let Some(row) = rows.next().await? {
        if let oxrdf::Term::Literal(body) = &row["body"] {
            qualified |= body.value().contains("at least one ex:Whole");
        }
    }
    assert!(
        qualified,
        "cross-source authored object-property type missing from diagram"
    );
    let output = tempfile::tempdir()?;
    let archive = output.path().join("retained.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    for file in ["index.html", "main.typ"] {
        let body =
            tokio::fs::read_to_string(publication.directory.join(file)).await?;
        assert!(
            body.contains("at least one"),
            "{file} lost a known cross-source object property declaration"
        );
    }
    Ok(())
}
