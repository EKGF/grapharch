#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        diagnostics::FailureMode,
        ontology_family,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};

async fn authored(renderer: &str) -> Result<String> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let text = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#renderer",
        definitions::literal(renderer),
    )?;
    tokio::fs::write(
        &path,
        format!(
            "{text}\nex:subject <http://www.w3.org/2000/01/rdf-schema#label> \
             \"First label\"@en, \"Other label\"@en .\n"
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    RdfFile::new(
        "urn:specification:project",
        "urn:quality-variants:source",
        path,
    )?
    .collect(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok(
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?,
    )
}

fn heading_label(heading: scraper::ElementRef<'_>) -> String {
    heading
        .children()
        .filter_map(|node| node.value().as_text())
        .map(|text| &**text)
        .collect()
}

fn report(html: &str, title: &str) {
    let document = scraper::Html::parse_document(html);
    let quality = document
        .select(&scraper::Selector::parse("#quality-issues").unwrap())
        .next()
        .expect("grouped quality report");
    assert_eq!(
        heading_label(
            quality
                .select(&scraper::Selector::parse("h2").unwrap())
                .next()
                .unwrap()
        ),
        title
    );
    assert!(
        quality
            .select(
                &scraper::Selector::parse(".quality-issue-group h3").unwrap()
            )
            .any(|h| { heading_label(h) == "Competing source annotations" })
    );
    assert!(quality.text().collect::<String>().contains("First label"));
    assert!(quality.text().collect::<String>().contains("Other label"));
}

#[tokio::test]
async fn generic_html_quality_report_follows_existing_appendices() -> Result<()>
{
    report(&authored("html").await?, "B. Quality issues");
    Ok(())
}

#[tokio::test]
async fn paginated_quality_report_follows_existing_appendices() -> Result<()> {
    report(
        &authored("specification-pages").await?,
        "B. Quality issues",
    );
    Ok(())
}

async fn bookshelf(failed_worker: bool) -> Result<String> {
    let source = tempfile::tempdir()?;
    let sources = source.path().join("sources");
    tokio::fs::create_dir(&sources).await?;
    tokio::fs::write(sources.join("vocabulary.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix sh:<http://www.w3.org/ns/shacl#> . @prefix ga:<https://ekgf.org/ontology/grapharch#> .
<https://example.org/core#> a owl:Ontology;rdfs:label "Core ontology"@en;sh:declare [sh:prefix "core";sh:namespace "https://example.org/core#"] .
<https://example.org/core#Term> a owl:Class;rdfs:label "First label"@en,"Other label"@en .
<urn:book> a ga:OntologyModule;rdfs:label "Core book"@en;ga:moduleMember <https://example.org/core#> .
"#).await?;
    let settings = source.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix gov:<https://ekgf.org/ontology/artifact-governance#> . @prefix pub:<https://ekgf.org/ontology/artifact-publication#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:project> a ga:Project;ga:target <urn:target> . <urn:scope> a ga:Scope . <urn:profile> a ga:Profile;pub:publishesFamily <urn:family> .
<urn:family> a gov:Family;rdfs:label "Example family"@en . <https://example.org/core#> gov:inFamily <urn:family> .
<urn:target> a ga:Target;dct:title "Example reference";dct:description "A family reference.";rdfs:comment "A test draft.";ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer "bookshelf";ga:destination "specification";ga:publicationStatus "Draft";ga:publicationDate "2026-10-25"^^<http://www.w3.org/2001/XMLSchema#date>;ga:thisVersion <https://example.org/spec/>;ga:latestVersion <https://example.org/spec/>;ga:publisher <urn:authority> .
<urn:authority> a ga:Authority;rdfs:label "Example Authority";ga:shortName "Example" .
"#).await?;
    let built = ontology_family::collect(
        &sources,
        &settings,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let executable = if failed_worker {
        std::path::PathBuf::from("/usr/bin/false")
    } else {
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_grapharch"))
    };
    let publisher = Publisher::new(restored)
        .with_failure_mode(FailureMode::KeepGoing)
        .with_book_workers(executable, 1)?;
    let PublishOutcome::Published(publication) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("publication required")
    };
    Ok(
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?,
    )
}

#[tokio::test]
async fn bookshelf_quality_groups_are_separate_from_execution_failures()
-> Result<()> {
    for failed_worker in [false, true] {
        let html = bookshelf(failed_worker).await?;
        report(&html, "Quality issues");
        let document = scraper::Html::parse_document(&html);
        let execution = document
            .select(
                &scraper::Selector::parse("#publication-diagnostics").unwrap(),
            )
            .next();
        if failed_worker {
            let execution = execution.expect("execution failure report");
            assert!(
                !execution
                    .text()
                    .collect::<String>()
                    .contains("Missing English source definition")
            );
            assert!(
                !execution
                    .text()
                    .collect::<String>()
                    .contains("competing-source-annotations")
            );
            assert!(
                execution
                    .text()
                    .collect::<String>()
                    .contains("book-render-failed")
            );
        } else {
            assert!(
                execution.is_none(),
                "quality warnings must not be repeated as execution failures"
            );
        }
    }
    Ok(())
}
