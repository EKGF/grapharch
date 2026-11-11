use {
    super::*,
    crate::publication::collect::{Collector, RdfFile},
};

const PROJECT: &str = "urn:test:project";
const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
                        .\n@prefix t: <urn:test:> .\n";
const CONTENT: &str = "t:abstract a ga:Fragment ; ga:subject t:abstract-topic \
                       .\nt:notes a ga:Fragment ; ga:subject t:notes-topic .\n";

async fn run(definitions: &str) -> Result<(Dataset, Result<()>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    let directory = tempfile::tempdir()?;
    for (processor, body) in [
        ("urn:test:definitions", definitions),
        ("urn:test:content", CONTENT),
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
    let result = bind(
        &dataset,
        &definitions,
        &NamedNode::new(PROJECT)?,
        &writer,
    )
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result))
}

#[tokio::test]
async fn included_uses_add_their_fragment_subjects_to_the_named_scope()
-> Result<()> {
    let (dataset, result) = run("t:project ga:collector t:abstract-source, \
                                 t:notes-source .\nt:scope a ga:Scope \
                                 .\nt:abstract-source a ga:CollectorUse ; \
                                 ga:fragment t:abstract ; ga:includedIn \
                                 t:scope .\nt:notes-source a ga:CollectorUse \
                                 ; ga:fragment t:notes .\n")
    .await?;
    result?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    let mut rows = view
        .objects(&NamedNode::new("urn:test:scope")?, "include")
        .await?;
    let mut included = Vec::new();
    while let Some(row) = rows.next().await? {
        included.push(row["value"].to_string());
    }
    assert_eq!(included, ["<urn:test:abstract-topic>"]);
    Ok(())
}

#[tokio::test]
async fn inclusions_need_a_contributed_fragment_and_a_declared_scope()
-> Result<()> {
    for (definitions, expected) in [
        (
            "t:project ga:collector t:use . t:scope a ga:Scope .\nt:use a \
             ga:CollectorUse ; ga:includedIn t:scope .",
            "ga:includedIn requires its ga:fragment",
        ),
        (
            "t:project ga:collector t:use . t:scope a ga:Scope .\nt:use a \
             ga:CollectorUse ; ga:fragment t:absent ; ga:includedIn t:scope .",
            "no collected fragment <urn:test:absent>",
        ),
        (
            "t:project ga:collector t:use .\nt:use a ga:CollectorUse ; \
             ga:fragment t:abstract ; ga:includedIn t:undeclared .",
            "references undefined ga:Scope",
        ),
    ] {
        let (_, result) = run(definitions).await?;
        let error = format!("{:#}", result.expect_err(definitions));
        assert!(error.contains(expected), "{expected}: {error}");
    }
    Ok(())
}
