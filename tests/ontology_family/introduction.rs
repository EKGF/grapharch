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
            qualified |= body.value().contains("at least one Whole");
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

#[tokio::test]
async fn range_only_placeholder_does_not_hide_a_pinned_cross_source_domain()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/classes.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/vocab/> a owl:Ontology; rdfs:label "Classes" .
<https://example.org/vocab/Target> a owl:Class; rdfs:label "Target" .
<https://example.org/vocab/PlaceholderTarget> a owl:Class; rdfs:label "Placeholder target" .
<https://example.org/vocab/q> a owl:ObjectProperty; rdfs:label "delegates control to"; rdfs:range <https://example.org/vocab/PlaceholderTarget> .
<https://example.org/vocab/p> rdfs:range <https://example.org/vocab/Target> .
"#).await?;
    tokio::fs::write(source.path().join("sources/declarations.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/vocab/p> a owl:ObjectProperty; rdfs:domain <https://example.org/vocab/Source> .
<https://example.org/vocab/Source> a owl:Class .
"#).await?;
    let built = collect(source.path()).await?;
    let mut rows = built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body WHERE { GRAPH ?g { ?fragment ga:subject <https://example.org/vocab/Target>; ga:body ?body } }".into()).await?;
    let mut found = false;
    while let Some(row) = rows.next().await? {
        if let oxrdf::Term::Literal(body) = &row["body"] &&
            body.value().contains("```grapharch-class-diagram")
        {
            found = true;
            assert!(
                !body.value().contains("unknownDomain"),
                "a collected specific domain must take priority over a \
                 placeholder"
            );
            assert!(body.value().contains("ex:Source"));
        }
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
        panic!("publication required")
    };
    for file in ["index.html", "main.typ"] {
        let text =
            tokio::fs::read_to_string(publication.directory.join(file)).await?;
        assert!(
            text.contains("presentation placeholder"),
            "missing accessible qualification in {file}"
        );
        assert!(text.contains("delegates control to"));
    }

    Ok(())
}

#[tokio::test]
async fn qualified_minimum_projection_survives_source_removal() -> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/ontology.ttl"), r#"
@prefix ex:<https://example.org/vocab/> .
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
ex: a owl:Ontology; rdfs:label "Qualified minimum fixture" .
ex:Department a owl:Class; rdfs:label "Department"; rdfs:subClassOf [a owl:Restriction; owl:onProperty ex:managedBy; owl:onClass ex:Minister; owl:minQualifiedCardinality 1] .
ex:OptionalDepartment a owl:Class; rdfs:label "Optional Department"; rdfs:subClassOf [a owl:Restriction; owl:onProperty ex:managedBy; owl:onClass ex:Minister; owl:minQualifiedCardinality 0] .
ex:Minister a owl:Class; rdfs:label "Minister" .
ex:managedBy a owl:ObjectProperty; rdfs:label "is managed by" .
"#).await?;
    let built = collect(source.path()).await?;
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
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let figures = scraper::Selector::parse("figure.owl-class-diagram").unwrap();
    let svgs = scraper::Selector::parse("svg text").unwrap();
    let mut positive = false;
    let mut zero = false;
    for figure in document.select(&figures) {
        let words = figure
            .select(&svgs)
            .flat_map(|n| n.text())
            .collect::<Vec<_>>()
            .join(" ");
        if words.contains("Optional Department") {
            zero = true;
            assert!(
                !words.contains("Minister") && !words.contains("expression")
            );
        } else if words.contains("Department") {
            positive = true;
            assert!(words.contains("1..*") && words.contains("qualified"));
        }
    }
    assert!(positive && zero);
    assert!(
        html.contains("minQualifiedCardinality"),
        "original details retained"
    );
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(
        typst.contains("qualified") &&
            typst.contains("minQualifiedCardinality")
    );
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    if let Some(directory) =
        std::env::var_os("GRAPHARCH_QUALIFIED_MINIMUM_OUTPUT")
    {
        let destination = std::path::PathBuf::from(directory);
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "main.typ", "document.pdf", "style.css"] {
            tokio::fs::copy(
                publication.directory.join(name),
                destination.join(name),
            )
            .await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn exact_count_and_nested_zero_proof_survive_source_removal() -> Result<()>
{
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/ontology.ttl"), r#"
@prefix ex:<https://example.org/vocab/> .
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
ex: a owl:Ontology; rdfs:label "Qualified exact fixture" .
ex:Official a owl:Class; rdfs:label "Official"; rdfs:subClassOf ex:ResponsibleParty,
 [a owl:Restriction; owl:onProperty ex:playedBy; owl:onClass ex:Person; owl:qualifiedCardinality 1],
 [a owl:Restriction; owl:onProperty ex:playedBy; owl:someValuesFrom [a owl:Restriction; owl:onProperty ex:member; owl:onClass ex:Body; owl:minQualifiedCardinality 0]] .
ex:ResponsibleParty a owl:Class; rdfs:label "Responsible party" .
ex:Appointee a owl:Class; rdfs:label "Appointee"; rdfs:subClassOf ex:Official .
ex:Person a owl:Class; rdfs:label "Person" .
ex:Body a owl:Class . ex:member a owl:ObjectProperty .
ex:playedBy a owl:ObjectProperty; rdfs:label "is played by" .
"#).await?;
    let built = collect(source.path()).await?;
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
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let figure_selector =
        scraper::Selector::parse("figure.owl-class-diagram").unwrap();
    let caption = scraper::Selector::parse("figcaption").unwrap();
    let svg = scraper::Selector::parse("svg text").unwrap();
    let figure = document
        .select(&figure_selector)
        .find(|f| {
            f.select(&caption).any(|c| {
                c.text()
                    .collect::<String>()
                    .starts_with("Class neighborhood: Official ")
            })
        })
        .unwrap();
    let visible = figure
        .select(&svg)
        .flat_map(|t| t.text())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(visible.contains("qualified") && visible.contains("is played by"));
    assert!(
        !visible.contains("expression") &&
            !visible.contains("Literal") &&
            !visible.contains("1..*")
    );
    for fact in [
        "someValuesFrom",
        "minQualifiedCardinality",
        "qualifiedCardinality",
    ] {
        assert!(html.contains(fact));
    }
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    if let Some(directory) =
        std::env::var_os("GRAPHARCH_QUALIFIED_EXACT_OUTPUT")
    {
        let destination = std::path::PathBuf::from(directory);
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "main.typ", "document.pdf", "style.css"] {
            tokio::fs::copy(
                publication.directory.join(name),
                destination.join(name),
            )
            .await?;
        }
    }
    Ok(())
}
