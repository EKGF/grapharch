use {
    super::*,
    crate::publication::collect::{Collector, RdfFile},
};

const PROJECT: &str = "urn:test:project";
const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> .\n\
                        @prefix dct: <http://purl.org/dc/terms/> .\n\
                        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\
                        @prefix t: <urn:test:> .\n";
const NATIVE: &str = r#"t:Ontology dct:title "Native title"@en ;
    dct:description "Native description"@en ;
    dct:license <https://example.org/license> ;
    dct:issued "2026-06-01"^^xsd:date ."#;
const SOURCE: &str = r#"t:source a ga:CollectorUse ; ga:subject t:Ontology .
t:spec a ga:Target ; ga:metadataSource t:source ;
    ga:derivedMetadata dct:title, dct:description, ga:license, ga:publicationDate ."#;

struct Fixture {
    dataset: Dataset,
    result:  Result<()>,
}

async fn run(
    definitions: &str,
    native: &str,
    related: &str,
) -> Result<Fixture> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    let directory = tempfile::tempdir()?;
    for (processor, body) in [
        ("urn:test:definitions", definitions),
        ("urn:test:source", native),
        ("urn:test:related", related),
    ] {
        let path = directory.path().join(format!(
            "{}.ttl",
            processor.rsplit(':').next().unwrap()
        ));
        std::fs::write(&path, format!("{PREFIXES}{body}"))?;
        RdfFile::new(PROJECT, processor, path)?
            .collect(&writer)
            .await?;
    }
    let definitions = RevisionView::new(
        dataset.clone(),
        dataset
            .snapshot_for_processor(PROJECT, "urn:test:definitions")
            .await?,
    );
    let result = derive(
        &dataset,
        &definitions,
        &NamedNode::new(PROJECT)?,
        &writer,
    )
    .await;
    drop(writer);
    worker.await??;
    Ok(Fixture { dataset, result })
}

async fn value(dataset: &Dataset, predicate: &str) -> Result<Option<Term>> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    view.optional_by(
        &NamedNode::new("urn:test:spec")?,
        &NamedNode::new(predicate)?,
    )
    .await
}

fn error(fixture: Fixture) -> String {
    match fixture.result {
        Ok(()) => panic!("invalid native metadata was accepted"),
        Err(error) => format!("{error:#}"),
    }
}

#[tokio::test]
async fn header_values_come_only_from_the_selected_source() -> Result<()> {
    let fixture = run(
        SOURCE,
        NATIVE,
        r#"t:Ontology dct:title "RELATED TITLE" ; dct:issued "1999-01-01"^^xsd:date ."#,
    )
    .await?;
    fixture.result?;
    for (predicate, expected) in [
        (
            "http://purl.org/dc/terms/title",
            "\"Native title\"@en",
        ),
        (
            "http://purl.org/dc/terms/description",
            "\"Native description\"@en",
        ),
        (
            "https://ekgf.org/ontology/grapharch#license",
            "<https://example.org/license>",
        ),
        (
            "https://ekgf.org/ontology/grapharch#publicationDate",
            "\"2026-06-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
        ),
    ] {
        assert_eq!(
            value(&fixture.dataset, predicate)
                .await?
                .map(|term| term.to_string())
                .as_deref(),
            Some(expected),
            "{predicate}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn explicit_missing_multiple_and_unmapped_values_are_errors() -> Result<()>
{
    let conflict = run(
        &format!("{SOURCE}\nt:spec dct:title \"Explicit title\" ."),
        NATIVE,
        "",
    )
    .await?;
    assert!(error(conflict).contains("conflicts with an explicit value"));
    let missing = run(
        SOURCE,
        r#"t:Ontology dct:title "Native title"@en ; dct:description "D"@en ; dct:license <https://example.org/license> ."#,
        "",
    )
    .await?;
    assert!(
        error(missing).contains("is missing <http://purl.org/dc/terms/issued>")
    );
    let multiple = run(
        SOURCE,
        &format!("{NATIVE}\nt:Ontology dct:title \"Second title\"@en ."),
        "",
    )
    .await?;
    assert!(
        error(multiple).contains("multiple <http://purl.org/dc/terms/title>")
    );
    let unmapped = run(
        r#"t:source a ga:CollectorUse ; ga:subject t:Ontology .
t:spec ga:metadataSource t:source ; ga:derivedMetadata ga:subtitle ."#,
        NATIVE,
        "",
    )
    .await?;
    assert!(error(unmapped).contains("has no native mapping"));
    let sourceless = run(
        "t:spec ga:derivedMetadata dct:title .",
        NATIVE,
        "",
    )
    .await?;
    assert!(error(sourceless).contains("requires one ga:metadataSource"));
    Ok(())
}
