use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family,
        render::{PublishOutcome, Publisher},
    },
    std::path::Path,
};

#[tokio::test]
async fn ekgf_family_publishes_documented_terms_from_restored_kg() -> Result<()>
{
    let source = tempfile::tempdir()?;
    copy(
        Path::new("examples/ekg-ontologies"),
        source.path(),
    )?;
    let built = ontology_family::collect(
        &source.path().join("sources"),
        &source.path().join("project.ttl"),
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await?;
    let mut conflated=built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?ontology WHERE {GRAPH ?source {?ontology a <http://www.w3.org/2002/07/owl#Ontology>} GRAPH ?bindings {?ontology a ga:Component}} LIMIT 1".into()).await?;
    assert!(
        conflated.next().await?.is_none(),
        "ontology identity must stay separate from documentation component \
         identity"
    );
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let mut diagnostics = restored
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT \
             ?diagnostic WHERE { GRAPH ?g { ?diagnostic a ga:Diagnostic ; \
             ga:code \"missing-source-definition\" } } LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        diagnostics.next().await?.is_none(),
        "the updated EKGF corpus must have no missing definitions"
    );
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("expected family publication")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    for text in [
        "EKGF Ontologies",
        "24 ontologies",
        "artifact-governance",
        "artifact-publication",
        "communication",
        "isPlayedBy",
    ] {
        assert!(html.contains(text), "missing {text}");
    }
    assert!(!html.contains("urn:grapharch:skolem:"));
    let doc = scraper::Html::parse_document(&html);
    assert!(
        doc.select(&scraper::Selector::parse("h2").unwrap())
            .any(|heading| {
                heading
                    .text()
                    .collect::<String>()
                    .contains("EKG Platform API Ontology")
            }),
        "ontology chapters must use their English names rather than namespace \
         prefixes"
    );
    let sections = scraper::Selector::parse(
        "main > section:not(#quality-issues):not(#publication-diagnostics)",
    )
    .unwrap();
    assert!(
        doc.select(&sections)
            .next_back()
            .unwrap()
            .text()
            .collect::<String>()
            .contains("Documentation coverage"),
        "coverage belongs after the ontology reference, not in the opening \
         navigation"
    );
    let overview_section = doc
        .select(&sections)
        .find(|section| {
            section
                .text()
                .collect::<String>()
                .contains("Explore the 24 ontologies")
        })
        .expect(
            "family overview must welcome readers with ontology navigation",
        );
    assert_eq!(
        overview_section
            .select(&scraper::Selector::parse("table").unwrap())
            .count(),
        0,
        "opening overview must not be a statistics table"
    );
    assert!(
        overview_section
            .select(&scraper::Selector::parse("li a").unwrap())
            .count() >=
            24
    );
    let count_headers = scraper::Selector::parse(
        "table:first-of-type thead th[style='text-align: right']",
    )
    .unwrap();
    assert!(
        doc.select(&count_headers).count() >= 2,
        "family resource counts must be right-aligned numeric columns"
    );
    let individual_heading = scraper::Selector::parse("h4").unwrap();
    assert!(
        doc.select(&individual_heading).any(|heading| {
            heading
                .text()
                .collect::<String>()
                .contains("Designing and managing data products")
        }),
        "named individuals need readable, explicitly classified headings"
    );
    assert!(
        html.contains("<strong>Instance of:</strong>"),
        "named individuals must expose their declared types as an instance \
         summary"
    );
    let links = scraper::Selector::parse("a[href^='#']").unwrap();
    for link in doc.select(&links) {
        let id = link.value().attr("href").unwrap().trim_start_matches('#');
        assert!(
            doc.select(&scraper::Selector::parse("[id]").unwrap())
                .any(|e| e.value().id() == Some(id)),
            "broken family link {id}"
        );
    }
    assert!(
        std::fs::read(publication.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
fn copy(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy(&entry.path(), &path)?;
        } else {
            std::fs::copy(entry.path(), path)?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn missing_annotation_warning_is_not_a_failed_operation() -> Result<()> {
    use grapharch::publication::{
        diagnostics::{Diagnostic, Severity},
        writer::GraphWriter,
    };
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    writer
        .record_diagnostic(
            "urn:family:project",
            "urn:family:processor",
            "urn:family:term",
            "document",
            Diagnostic {
                severity:        Severity::Warning,
                code:            "missing-source-definition".into(),
                message:         "No source definition".into(),
                reference:       "urn:family:term".into(),
                source_fragment: None,
            },
        )
        .await?;
    drop(writer);
    worker.await??;
    let mut rows = dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?status \
             WHERE {GRAPH ?g {?operation a ga:Operation ; ga:status ?status}}"
                .into(),
        )
        .await?;
    let row = rows.next().await?.unwrap();
    let oxrdf::Term::Literal(status) = &row["status"] else {
        panic!("status literal required")
    };
    assert_eq!(status.value(), "completed-with-diagnostics");
    Ok(())
}

#[tokio::test]
async fn unavailable_registered_member_fails_instead_of_publishing_a_partial_family()
-> Result<()> {
    let source = tempfile::tempdir()?;
    copy(
        Path::new("examples/ekg-ontologies"),
        source.path(),
    )?;
    std::fs::remove_file(source.path().join("sources/ekgf-api.ttl"))?;
    let result = ontology_family::collect(
        &source.path().join("sources"),
        &source.path().join("project.ttl"),
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await;
    assert!(
        result.is_err(),
        "a declared family member cannot silently disappear from the \
         publication"
    );
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("no loaded ontology source")
    );
    Ok(())
}

#[test]
fn family_fixture_native_sources_match_the_pinned_manifest() -> Result<()> {
    use sha2::{Digest, Sha256};
    let manifest: serde_yaml::Value = serde_yaml::from_str(include_str!(
        "../examples/ekg-ontologies/reference.yaml"
    ))?;
    for (path, digest) in manifest["sources"].as_mapping().unwrap() {
        let bytes = std::fs::read(
            Path::new("examples/ekg-ontologies/sources")
                .join(path.as_str().unwrap()),
        )?;
        assert_eq!(
            hex::encode(Sha256::digest(bytes)),
            digest.as_str().unwrap()
        );
    }
    Ok(())
}

#[tokio::test]
async fn source_diagnostic_retains_the_observed_revision_when_project_heads_advance()
-> Result<()> {
    use grapharch::publication::{
        diagnostics::{Diagnostic, Severity, SourceContext},
        writer::GraphWriter,
    };
    let dataset = Dataset::memory()?;
    let observed = dataset.snapshot_for("urn:family:project").await?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let mut changed = writer
        .begin(
            "urn:family:project",
            "urn:family:collector",
            "urn:family:source",
            "one",
        )
        .await?;
    changed
        .write(oxrdf::Quad::new(
            oxrdf::NamedNode::new("urn:family:subject")?,
            oxrdf::NamedNode::new("urn:family:predicate")?,
            oxrdf::Literal::new_simple_literal("later input"),
            oxrdf::GraphName::DefaultGraph,
        ))
        .await?;
    changed.complete().await?;
    assert_ne!(
        dataset.snapshot_for("urn:family:project").await?,
        observed
    );
    writer
        .record_diagnostic_at(
            SourceContext {
                project:        "urn:family:project",
                processor:      "urn:family:documentor",
                source:         "urn:family:term",
                stage:          "document",
                input_revision: Some(&observed),
            },
            Diagnostic {
                severity:        Severity::Warning,
                code:            "missing-source-definition".into(),
                message:         "No definition in observed source".into(),
                reference:       "urn:family:term".into(),
                source_fragment: None,
            },
        )
        .await?;
    drop(writer);
    worker.await??;
    let mut rows = dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT \
             ?revision WHERE {GRAPH ?g {?diagnostic a ga:Diagnostic ; \
             ga:inputRevision ?revision}}"
                .into(),
        )
        .await?;
    assert_eq!(
        rows.next().await?.unwrap()["revision"],
        oxrdf::Term::NamedNode(observed)
    );
    Ok(())
}

#[tokio::test]
async fn family_accepts_native_rdfxml_and_namespace_hints_without_parsing_xml_catalogs()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("sources");
    std::fs::create_dir(&source)?;
    std::fs::write(
        source.join("Finance.rdf"),
        r#"<?xml version="1.0"?><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:owl="http://www.w3.org/2002/07/owl#" xmlns:rdfs="http://www.w3.org/2000/01/rdf-schema#" xmlns:skos="http://www.w3.org/2004/02/skos/core#"><owl:Ontology rdf:about="https://example.org/finance/"><rdfs:label xml:lang="en">Finance ontology</rdfs:label><owl:imports rdf:resource="https://unresolved.invalid/external/"/></owl:Ontology><owl:Class rdf:about="https://example.org/finance/Account"><rdfs:label xml:lang="en">Account</rdfs:label><skos:definition xml:lang="en">A supplied account definition.</skos:definition><rdfs:subClassOf><owl:Restriction><owl:onProperty rdf:resource="https://example.org/finance/owns"/><owl:someValuesFrom rdf:resource="https://example.org/finance/Account"/></owl:Restriction></rdfs:subClassOf></owl:Class><owl:ObjectProperty rdf:about="https://example.org/finance/owns"><rdfs:label xml:lang="en">owns</rdfs:label><rdfs:comment xml:lang="en">A supplied ownership description.</rdfs:comment></owl:ObjectProperty></rdf:RDF>"#,
    )?;
    std::fs::write(
        source.join("catalog-v001.xml"),
        "<catalog xmlns=\"urn:oasis:names:tc:entity:xmlns:xml:catalog\"/>",
    )?;
    let definitions = root.path().join("project.ttl");
    std::fs::write(
        &definitions,
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix artgov:<https://ekgf.org/ontology/artifact-governance#> . @prefix artpub:<https://ekgf.org/ontology/artifact-publication#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix dct:<http://purl.org/dc/terms/> . @prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:test:project> a ga:Project ; ga:target <urn:test:target> . <urn:test:scope> a ga:Scope . <urn:test:profile> a ga:Profile ; artpub:publishesFamily <urn:test:family> . <urn:test:family> a artgov:Family ; rdfs:label "Finance family"@en . <urn:test:reference> a artgov:Reference ; artgov:inFamily <urn:test:family> ; artgov:refersTo <https://example.org/finance/> . <https://example.org/finance/> sh:declare [ sh:prefix "fin" ; sh:namespace "https://example.org/finance/" ] .
<urn:test:target> a ga:Target ; dct:title "Finance reference" ; dct:description "A native RDF/XML family test." ; rdfs:comment "Test preview." ; ga:scope <urn:test:scope> ; ga:profile <urn:test:profile> ; ga:renderer "specification" ; ga:destination "specification" ."#,
    )?;
    let built = ontology_family::collect(
        &source,
        &definitions,
        "urn:test:project",
        "urn:test:target",
        2,
    )
    .await?;
    assert_eq!(built.ontology_count, 1);
    assert_eq!(built.term_count, 2);
    let mut bodies = built
        .dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body \
             WHERE {GRAPH ?g {?fragment a ga:Fragment ; ga:body ?body} \
             FILTER(CONTAINS(STR(?body),'A supplied account definition.'))} \
             LIMIT 1"
                .into(),
        )
        .await?;
    let row = bodies.next().await?.unwrap();
    let oxrdf::Term::Literal(body) = &row["body"] else {
        panic!("body required")
    };
    assert!(body.value().contains("owl:someValuesFrom"));
    assert!(!body.value().contains("urn:grapharch:skolem:"));
    Ok(())
}

#[test]
fn offline_family_example_uses_the_registered_family_name() -> Result<()> {
    use oxrdf::Term;
    let titles = oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_reader(
            include_bytes!("../examples/ekg-ontologies/project.ttl").as_slice(),
        )
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|quad| {
            quad.predicate.as_str() == "http://purl.org/dc/terms/title"
        })
        .map(|quad| quad.object)
        .collect::<Vec<_>>();
    assert_eq!(titles.len(), 2);
    assert!(titles.iter().all(|title| matches!(title, Term::Literal(value) if value.value() == "EKGF Ontologies")));
    Ok(())
}
