use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family,
        render::{PublishOutcome, Publisher},
    },
};

async fn render(
    term: Option<&str>,
    renderer: &str,
) -> Result<(tempfile::TempDir, String)> {
    let source = tempfile::tempdir()?;
    let corpus = source.path().join("sources");
    tokio::fs::create_dir(&corpus).await?;
    tokio::fs::write(corpus.join("ontology.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
<https://example.org/vocab/> a owl:Ontology; rdfs:label "Example vocabulary"; dct:description "An example ontology." .
<https://example.org/vocab/Thing> a owl:Class; rdfs:label "Thing"; dct:description "An example thing." .
"#).await?;
    let hint = term
        .map(|term| {
            format!("<urn:terms:collection> ga:collectionTerm \"{term}\" .")
        })
        .unwrap_or_default();
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, format!(r##"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:terms:project> a ga:Project; dct:title "Example vocabularies"; ga:target <urn:terms:target> .
<urn:terms:scope> a ga:Scope .
<urn:terms:profile> a ga:Profile; pub:publishesFamily <urn:terms:collection> .
<urn:terms:collection> a gov:Family; rdfs:label "Example vocabularies" .
{hint}
<urn:terms:target> a ga:Target; dct:title "Example vocabularies"; dct:description "Collection reference."; rdfs:comment "Unofficial preview."; ga:scope <urn:terms:scope>; ga:profile <urn:terms:profile>; ga:renderer "{renderer}"; ga:destination "reference"; ga:publisher <urn:terms:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:terms:publisher> a gov:Authority; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://example.org/vocab/> gov:inFamily <urn:terms:collection>; sh:declare [sh:prefix "example"; sh:namespace "https://example.org/vocab/"] .
"##)).await?;
    let built = ontology_family::collect(
        &corpus,
        &definitions,
        "urn:terms:project",
        "urn:terms:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored).with_book_workers(
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_grapharch")),
        1,
    )?;
    let PublishOutcome::Published(publication) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    Ok((output, html))
}

#[tokio::test]
async fn collection_hint_changes_overview_and_pdf_without_changing_semantics()
-> Result<()> {
    let (output, html) = render(Some("library"), "specification").await?;
    assert!(html.contains("Library overview"));
    assert!(html.contains("in this library"));
    assert!(!html.contains("Family overview"));
    let typst =
        tokio::fs::read_to_string(output.path().join("reference/main.typ"))
            .await?;
    assert!(
        typst.contains("Library overview") && typst.contains("in this library")
    );
    assert!(
        tokio::fs::read(output.path().join("reference/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn paginated_navigation_and_status_labels_follow_collection_term()
-> Result<()> {
    let (output, html) =
        render(Some("ontology suite"), "specification-pages").await?;
    assert!(html.contains("Browse this ontology suite"));
    assert!(html.contains("Ontology suite overview"));
    assert!(html.contains("Loading ontology suite navigation"));
    assert!(html.contains("Ontology suite navigation unavailable"));
    assert!(!html.contains("Browse this family"));
    let search = tokio::fs::read_to_string(
        output.path().join("reference/search-index.json"),
    )
    .await?;
    assert!(search.contains("Ontology suite overview"));
    Ok(())
}

#[tokio::test]
async fn default_collection_term_remains_family() -> Result<()> {
    let (_, html) = render(None, "specification-pages").await?;
    assert!(html.contains("Browse this family"));
    assert!(html.contains("Family overview"));
    assert!(html.contains("in this family"));
    Ok(())
}

#[tokio::test]
async fn bookshelf_search_uses_the_same_collection_hint() -> Result<()> {
    let (output, html) = render(Some("library"), "bookshelf").await?;
    assert!(!html.contains("Family overview"));
    let search = tokio::fs::read_to_string(
        output.path().join("reference/search-index.json"),
    )
    .await?;
    assert!(search.contains("Library overview"));
    assert!(!search.contains("Family overview"));
    Ok(())
}

#[tokio::test]
async fn ontology_reference_explains_missing_definition_origin_after_restore()
-> Result<()> {
    let (output, html) = render(None, "specification").await?;
    assert!(html.contains("rdfs:isDefinedBy"));
    assert!(html.contains("Not declared in collected sources."));
    let typst =
        tokio::fs::read_to_string(output.path().join("reference/main.typ"))
            .await?;
    assert!(typst.contains("Not declared in collected sources."));
    if let Some(directory) = std::env::var_os("GRAPHARCH_DEFINED_BY_REVIEW") {
        let directory = std::path::PathBuf::from(directory);
        tokio::fs::create_dir_all(&directory).await?;
        for filename in ["index.html", "style.css", "main.typ", "document.pdf"]
        {
            tokio::fs::copy(
                output.path().join("reference").join(filename),
                directory.join(filename),
            )
            .await?;
        }
    }
    Ok(())
}
