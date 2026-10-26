#[path = "support/definitions.rs"]
mod definitions;

use {anyhow::Result, std::path::Path};

fn docs(root: &Path) -> Result<std::path::PathBuf> {
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs)?;
    std::fs::write(
        docs.join("grapharch.ttl"),
        definitions::edit_definitions(
            include_str!("../docs/grapharch.ttl"),
            &["ontologySource"],
            None,
        )?,
    )?;
    std::fs::write(
        docs.join("index.md"),
        "---\ntitle: Gallery\ngrapharch:\n  subject: urn:gallery:page\n  \
         showcase_gallery: true\n---\n# Gallery\n",
    )?;
    Ok(docs)
}
fn registry(root: &Path, cases: &str) -> Result<()> {
    std::fs::create_dir_all(root.join("examples"))?;
    std::fs::write(
        root.join("examples/showcases.ttl"),
        format!(
            "@prefix ga:<https://ekgf.org/ontology/grapharch#> \
             .\n<urn:grapharch:docs> ga:showcase \
             <urn:case:first>,<urn:case:second> .\n{cases}"
        ),
    )?;
    Ok(())
}

#[tokio::test]
async fn website_reports_every_failed_showcase_instead_of_abandoning_later_cases()
-> Result<()> {
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:kind "authored";ga:source "missing-first.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:kind "authored";ga:source "missing-second.ttl" .
"#,
    )?;
    let docs = docs(root.path())?;
    let failure = grapharch::publication::website::collect(
        &docs,
        &docs.join("grapharch.ttl"),
        "urn:grapharch:docs",
        "urn:grapharch:docs:website",
        2,
    )
    .await
    .err()
    .expect("both invalid cases must fail the publication");
    let report = format!("{failure:#}");
    assert!(
        report.contains("urn:case:first") && report.contains("urn:case:second"),
        "all cases must run and identify their failures: {report}"
    );
    Ok(())
}

#[tokio::test]
async fn verification_keeps_healthy_case_and_error_kg_when_another_case_fails()
-> Result<()> {
    use grapharch::publication::{dataset::Dataset, showcases};
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:kind "authored";ga:source "missing.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:kind "authored";ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/document.pdf" .
"#,
    )?;
    std::fs::copy(
        "examples/showcase/project.ttl",
        root.path().join("examples/authored.ttl"),
    )?;
    let output = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let observed = dataset.clone();
    let report = showcases::verify_gallery(
        dataset,
        &root.path().join("examples"),
        "urn:grapharch:docs",
        output.path(),
    )
    .await?;
    assert!(!report.succeeded);
    assert!(report.ensure_success().is_err());
    assert_eq!(report.cases.len(), 2);
    assert!(!report.cases[0].succeeded);
    assert_eq!(report.cases[0].source, "missing.ttl");
    assert!(
        report.cases[0]
            .error
            .as_deref()
            .is_some_and(|error| !error.trim().is_empty())
    );
    assert!(report.cases[0].dataset_archive.is_none());
    assert!(
        report.cases[1].succeeded,
        "healthy later case must finish despite earlier failure: {:?}",
        report.cases[1].error
    );
    assert!(report.cases[1].dataset_archive.as_ref().unwrap().is_file());
    assert!(
        output
            .path()
            .join("artifacts/second/specification/index.html")
            .is_file()
    );
    assert!(output.path().join("report.json").is_file());
    let mut bindings = observed
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?asset \
             WHERE {GRAPH ?g {<urn:case:second> ga:previewArtifact ?asset}} \
             LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        bindings.next().await?.is_some(),
        "completed healthy-case bindings must be committed for publication"
    );
    let mut failures = observed
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?error \
             WHERE {GRAPH ?g {?error a ga:Diagnostic;ga:code \
             \"showcase-build-failed\"}} LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        failures.next().await?.is_some(),
        "unexpected case failures must be recorded in the caller's KG"
    );
    let restored = Dataset::memory()?;
    restored.import(report.dataset_archive.unwrap()).await?;
    let mut failures = restored
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?error \
             WHERE {GRAPH ?g {?error a ga:Diagnostic;ga:code \
             \"showcase-build-failed\"}} LIMIT 1"
                .into(),
        )
        .await?;
    assert!(failures.next().await?.is_some());
    assert!(
        showcases::verify_gallery(
            Dataset::memory()?,
            &root.path().join("examples"),
            "urn:grapharch:docs",
            output.path()
        )
        .await
        .is_err(),
        "a previous report directory must not become stale evidence for a new \
         run"
    );
    Ok(())
}

#[tokio::test]
async fn collected_case_kg_survives_a_later_verification_failure() -> Result<()>
{
    use grapharch::publication::{dataset::Dataset, showcases};
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:kind "authored";ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/document.pdf";ga:expectedDiagnostic "expected-but-absent" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:kind "authored";ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/document.pdf" .
"#,
    )?;
    std::fs::copy(
        "examples/showcase/project.ttl",
        root.path().join("examples/authored.ttl"),
    )?;
    let output = tempfile::tempdir()?;
    let report = showcases::verify_gallery(
        Dataset::memory()?,
        &root.path().join("examples"),
        "urn:grapharch:docs",
        output.path(),
    )
    .await?;
    assert!(!report.succeeded);
    assert!(!report.cases[0].succeeded);
    assert!(
        report.cases[0]
            .error
            .as_ref()
            .unwrap()
            .contains("expected-but-absent")
    );
    let observed = Dataset::memory()?;
    observed
        .import(report.cases[0].dataset_archive.clone().expect(
            "collected evidence must survive subsequent case verification \
             failure",
        ))
        .await?;
    let mut fragments = observed
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT \
             ?fragment WHERE {GRAPH ?g {?fragment a ga:Fragment}} LIMIT 1"
                .into(),
        )
        .await?;
    assert!(fragments.next().await?.is_some());
    assert!(report.cases[1].succeeded);
    Ok(())
}
