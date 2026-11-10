use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

async fn source(selected: &str) -> Result<tempfile::TempDir> {
    let root = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\n---\n# \
         Home\n[Spec](spec/index.html)\n",
    )
    .await?;
    tokio::fs::write(root.path().join("owned.ttl"), r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:owned> a owl:Ontology ; <http://purl.org/vocab/vann/preferredNamespaceUri> "urn:owned:" ; <http://purl.org/vocab/vann/preferredNamespacePrefix> "owned" ; dct:description "Owned ontology abstract." ; <http://www.w3.org/2004/02/skos/core#scopeNote> "Draft." ; rdfs:comment "Owned model." .
<urn:owned:Thing> a owl:Class ; rdfs:isDefinedBy <urn:owned> ; rdfs:label "Owned thing"@en ; dct:description "Owned definition."@en ; rdfs:subClassOf <https://example.org/external#Role> .
"#).await?;
    tokio::fs::write(root.path().join("external.ttl"), r#"
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/external#Role> a <http://www.w3.org/2002/07/owl#Class> ; rdfs:label "External role"@en ; rdfs:comment "Original external role definition."@en ; rdfs:isDefinedBy <https://example.org/external> .
<http://www.w3.org/ns/prov#wasDerivedFrom> a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .
<https://example.org/external#Unselected> a <http://www.w3.org/2002/07/owl#Class> ; rdfs:label "Unselected external class"@en ; rdfs:comment "DO NOT PUBLISH THIS DEFINITION"@en .
"#).await?;
    tokio::fs::write(root.path().join("properties.ttl"), r#"
<http://www.w3.org/ns/prov#wasDerivedFrom> <http://www.w3.org/2000/01/rdf-schema#label> "was derived from" ; <http://www.w3.org/ns/prov#definition> "Original PROV derivation definition."@en .
"#).await?;
    tokio::fs::write(root.path().join("typing-only.ttl"), "<https://example.org/external#Role> a <http://www.w3.org/2002/07/owl#Class> .").await?;
    tokio::fs::write(root.path().join("project.ttl"), format!(r##"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:p> a ga:Project ; dct:title "Dependency test" ; ga:target <urn:site>, <urn:spec> ; ga:scope <urn:site-scope> ; ga:collector <urn:external>, <urn:properties>, <urn:typing-only> .
<urn:external> a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector ; ga:source "external.ttl" .
<urn:properties> a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector ; ga:source "properties.ttl" .
<urn:typing-only> a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector ; ga:source "typing-only.ttl" .
<urn:site-scope> a ga:Scope . <urn:site-profile> a ga:Profile .
<urn:site> a ga:Target ; dct:title "Site" ; ga:scope <urn:site-scope> ; ga:profile <urn:site-profile> ; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer> ; ga:destination "site" .
<urn:spec-scope> a ga:Scope ; ga:include {selected} . <urn:spec-profile> a ga:Profile .
<urn:spec> a ga:Target ; dct:title "Owned specification" ; ga:ontologySource "owned.ttl" ; ga:scope <urn:spec-scope> ; ga:profile <urn:spec-profile> ; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer> ; ga:destination "spec" ; ga:publisher <urn:authority> ; ga:publicationStatus "Draft" ; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date> ; ga:thisVersion <https://example.org/spec> ; ga:latestVersion <https://example.org/spec> .
<urn:authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example Authority" ; ga:shortName "Example" ; ga:accentColor "#123456" .
"##)).await?;
    Ok(root)
}

#[tokio::test]
async fn explicitly_selected_reused_terms_survive_restoration() -> Result<()> {
    let root = source("<https://example.org/external#Role>, <http://www.w3.org/ns/prov#wasDerivedFrom>").await?;
    let built = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:p",
        "urn:site",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(root.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html = tokio::fs::read_to_string(
        publication.directory.join("spec/index.html"),
    )
    .await?;
    for expected in [
        "Reused vocabulary",
        "Original external role definition.",
        "Original PROV derivation definition.",
        "external.ttl",
        "properties.ttl",
        "https://example.org/external#Role",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    assert!(!html.contains("DO NOT PUBLISH THIS DEFINITION"));
    assert!(
        !html.contains("typing-only.ttl"),
        "a typing-only source must not receive definition credit"
    );
    let document = scraper::Html::parse_document(&html);
    let local_links = scraper::Selector::parse("a[href^='#']").unwrap();
    let link = document
        .select(&local_links)
        .find(|link| link.text().collect::<String>().contains("Role"))
        .expect("owned term links to selected external role");
    let target = link.value().attr("href").unwrap().trim_start_matches('#');
    assert!(
        document
            .select(&scraper::Selector::parse("[id]").unwrap())
            .any(|node| node.value().id() == Some(target))
    );

    let typst =
        tokio::fs::read_to_string(publication.directory.join("spec/main.typ"))
            .await?;
    assert!(typst.contains("Original external role definition."));
    assert!(typst.contains("Original PROV derivation definition."));
    assert!(typst.contains("external.ttl"));
    assert!(!typst.contains("DO NOT PUBLISH THIS DEFINITION"));
    assert!(
        tokio::fs::read(publication.directory.join("spec/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn a_selected_term_missing_from_explicit_sources_fails() -> Result<()> {
    let root = source("<https://example.org/missing>").await?;
    let failure = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:p",
        "urn:site",
        2,
    )
    .await
    .err()
    .expect("missing selected term must fail");
    assert!(format!("{failure:#}").contains("selected"));
    Ok(())
}

#[tokio::test]
async fn selected_terms_reject_unsupported_or_incompatible_types() -> Result<()>
{
    for types in ["<http://www.w3.org/2002/07/owl#NamedIndividual>", "<http://www.w3.org/2002/07/owl#Class>, <http://www.w3.org/2002/07/owl#ObjectProperty>"] {
        let root = source("<https://example.org/external#Invalid>").await?;
        tokio::fs::write(root.path().join("external.ttl"), format!("<https://example.org/external#Invalid> a {types} .")).await?;
        let failure = website::collect(root.path(), &root.path().join("project.ttl"), "urn:p", "urn:site", 2).await.err().expect("invalid selected type must fail");
        assert!(format!("{failure:#}").contains("selected term"));
    }
    Ok(())
}
