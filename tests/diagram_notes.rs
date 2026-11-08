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
<https://example.org/vocab/Thing> a owl:Class; rdfs:label "Thing"; dct:description "An example thing."; rdfs:subClassOf <https://example.org/vocab/Parent> .
<https://example.org/vocab/Parent> a owl:Class; rdfs:label "Parent"; dct:description "The parent." .
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
async fn restored_diagrams_share_one_final_note_per_document() -> Result<()> {
    for renderer in ["specification", "specification-pages", "bookshelf"] {
        let (output, _) = render(None, renderer).await?;
        let root = output.path().join("reference");
        let mut files = tokio::fs::read_dir(&root).await?;
        let mut notes = 0;
        let mut refs = 0;
        while let Some(file) = files.next_entry().await? {
            if file.path().extension().is_some_and(|e| e == "html") {
                let body = tokio::fs::read_to_string(file.path()).await?;
                notes += body.matches("id=\"grapharch-diagram-note\"").count();
                refs += body.matches("role=\"doc-noteref\"").count();
            }
        }
        assert_eq!(notes, 1, "{renderer}: one shared note");
        assert!(
            refs >= 2,
            "{renderer}: every class diagram references the note"
        );
        let mut files = tokio::fs::read_dir(&root).await?;
        let mut documents = 0;
        while let Some(file) = files.next_entry().await? {
            if file.path().extension().is_some_and(|e| e == "typ") {
                let typst = tokio::fs::read_to_string(file.path()).await?;
                let references = typst
                    .matches("#link(label(\"grapharch-diagram-note\"))")
                    .count();
                assert_eq!(
                    typst.matches("<grapharch-diagram-note>").count(),
                    usize::from(references > 0)
                );
                if references > 0 {
                    documents += 1;
                    assert!(
                        typst.rfind("<grapharch-diagram-note>").unwrap() >
                            typst
                                .rfind(
                                    "#link(label(\"grapharch-diagram-note\"))"
                                )
                                .unwrap()
                    );
                }
            }
        }
        if renderer == "specification" || renderer == "bookshelf" {
            assert!(documents > 0, "{renderer}: PDF note");
        }
    }
    Ok(())
}

#[tokio::test]
async fn website_has_one_global_note_after_source_removal() -> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("ontology.ttl"), r#"@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<urn:one> a owl:Class; rdfs:label "One"; rdfs:subClassOf <urn:two> .
<urn:two> a owl:Class; rdfs:label "Two" ."#).await?;
    let output = tempfile::tempdir()?;
    let request = grapharch::publication::quick_build::BuildRequest {
        source: source.path().into(),
        output: output.path().join("before"),
    };
    let (dataset, plan) = request.collect().await?;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, &output.path().join("after"))
        .await?
    else {
        panic!("published")
    };
    let note = tokio::fs::read_to_string(
        publication.directory.join("diagram-notes.html"),
    )
    .await?;
    assert_eq!(
        note.matches("id=\"grapharch-diagram-note\"").count(),
        1
    );
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert_eq!(html.matches("role=\"doc-noteref\"").count(), 2);
    assert!(
        html.contains("href=\"diagram-notes.html#grapharch-diagram-note\"")
    );
    assert!(!html.contains("These diagrams present asserted OWL"));
    Ok(())
}
