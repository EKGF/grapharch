//! Definitions that use a term of the GraphArch namespace the engine does not
//! declare are rejected: they need a newer GraphArch, or contain a mistake.
use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::Dataset,
        writer::GraphWriter,
    },
    std::sync::Arc,
};

const PROJECT: &str = "urn:test:project";

const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> .\n\
                        @prefix dct: <http://purl.org/dc/terms/> .\n";

async fn assemble(definitions: &str) -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(&path, format!("{PREFIXES}{definitions}")).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    collect_all(
        vec![Arc::new(RdfFile::new(PROJECT, "urn:test:collector", path)?)
            as Arc<dyn Collector>],
        &writer,
        1,
    )
    .await?;
    drop(writer);
    worker.await??;
    Assembler::new(dataset).assemble(PROJECT).await?;
    Ok(())
}

fn rejection(result: Result<()>) -> String {
    format!(
        "{:#}",
        result.expect_err("definitions must be rejected")
    )
}

#[tokio::test]
async fn unknown_predicate_is_rejected() -> Result<()> {
    let message = rejection(
        assemble("<urn:test:project> a ga:Project ; ga:pairedPdfx false .")
            .await,
    );
    assert!(message.contains("ga:pairedPdfx"), "{message}");
    assert!(message.contains("predicate"), "{message}");
    assert!(message.contains("project.ttl"), "{message}");
    assert!(
        message.contains(env!("CARGO_PKG_VERSION")),
        "{message}"
    );
    assert!(message.contains("newer GraphArch"), "{message}");
    Ok(())
}

#[tokio::test]
async fn unknown_class_is_rejected() -> Result<()> {
    let message = rejection(
        assemble(
            "<urn:test:project> a ga:Project .\n<urn:test:extra> a ga:Targett \
             .",
        )
        .await,
    );
    assert!(message.contains("ga:Targett"), "{message}");
    assert!(message.contains("class"), "{message}");
    Ok(())
}

#[tokio::test]
async fn unknown_individual_is_rejected() -> Result<()> {
    let message = rejection(
        assemble("<urn:test:project> a ga:Project ; ga:theme ga:NoSuchTheme .")
            .await,
    );
    assert!(message.contains("ga:NoSuchTheme"), "{message}");
    assert!(message.contains("individual"), "{message}");
    Ok(())
}

#[tokio::test]
async fn known_terms_still_pass() -> Result<()> {
    assemble(
        "<urn:test:project> a ga:Project ; ga:theme ga:ReaderTheme \
         ;\nga:pairedPdf false .",
    )
    .await
}

#[tokio::test]
async fn foreign_mentions_of_unknown_terms_are_content() -> Result<()> {
    assemble(
        "<urn:test:project> a ga:Project ;\n\
         dct:references <https://ekgf.org/ontology/grapharch#Retired> .",
    )
    .await
}

fn validate(definitions: &str) -> Result<std::process::Output> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("definitions.ttl");
    std::fs::write(&path, format!("{PREFIXES}{definitions}"))?;
    Ok(
        std::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
            .args(["publication", "validate", "--definitions"])
            .arg(&path)
            .output()?,
    )
}

#[test]
fn standalone_validation_rejects_an_unknown_term_and_names_the_file()
-> Result<()> {
    let output =
        validate("<urn:test:project> a ga:Project ; ga:noSuchTerm 1 .")?;
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("ga:noSuchTerm"), "{message}");
    assert!(message.contains("definitions.ttl"), "{message}");
    assert!(message.contains("newer GraphArch"), "{message}");
    Ok(())
}

#[test]
fn standalone_validation_accepts_known_terms() -> Result<()> {
    let output = validate("<urn:test:project> a ga:Project .")?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}
