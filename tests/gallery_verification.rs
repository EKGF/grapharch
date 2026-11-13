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
            &[
                "ontologySource",
                "navigation",
                "layout",
                "collector",
                "include",
            ],
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

#[test]
fn reduced_gallery_fixture_does_not_activate_external_collectors() -> Result<()>
{
    let root = tempfile::tempdir()?;
    let docs = docs(root.path())?;
    let bytes = std::fs::read(docs.join("grapharch.ttl"))?;
    let mut active = false;
    for quad in oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(&bytes)
    {
        let quad = quad?;
        active |= quad.subject.to_string() == "<urn:grapharch:docs>" &&
            quad.predicate.as_str() ==
                "https://ekgf.org/ontology/grapharch#collector";
    }
    assert!(
        !active,
        "reduced gallery fixture must not run unrelated remote ontology \
         dependencies"
    );
    Ok(())
}

#[tokio::test]
async fn website_reports_every_failed_showcase_instead_of_abandoning_later_cases()
-> Result<()> {
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "missing-first.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "missing-second.ttl" .
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
async fn showcase_kind_literals_are_rejected_instead_of_selecting_a_runner()
-> Result<()> {
    for selector in [
        r#"ga:kind "authored""#,
        r#"ga:showcaseRunner "authored""#,
        "ga:showcaseRunner ga:UnknownShowcaseRunner",
    ] {
        let root = tempfile::tempdir()?;
        registry(
            root.path(),
            &format!(
                r#"
<urn:case:first> a ga:Showcase;ga:slug "first";{selector};ga:source "authored.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "authored.ttl" .
"#
            ),
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
        .unwrap_or_else(|| panic!("{selector} must not select a runner"));
        let report = format!("{failure:#}");
        assert!(
            report.contains("showcaseRunner"),
            "{selector}: {report}"
        );
    }
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
<urn:case:first> a ga:Showcase;ga:slug "first";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "missing.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/example-spec-showcase.pdf" .
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
<urn:case:first> a ga:Showcase;ga:slug "first";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/example-spec-showcase.pdf";ga:expectedDiagnostic "expected-but-absent" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:AuthoredShowcaseRunner;ga:source "authored.ttl";ga:previewPath "specification/index.html";ga:pdfPath "review/example-spec-showcase.pdf" .
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

#[tokio::test]
async fn verification_fails_a_showcase_whose_relative_links_name_unpublished_files()
-> Result<()> {
    use grapharch::publication::{dataset::Dataset, showcases};
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:showcaseRunner ga:SpecificationShowcaseRunner;ga:source "broken.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:SpecificationShowcaseRunner;ga:source "portable.ttl" .
"#,
    )?;
    for (name, links) in [
        (
            "broken.ttl",
            r#"<a href="../pdf/omg/document.pdf">OMG edition</a>"#,
        ),
        (
            "portable.ttl",
            r#"<a href="ekgf-spec-specification.pdf">Specification</a><br><a href="./">Online</a>"#,
        ),
    ] {
        std::fs::write(
            root.path().join("examples").join(name),
            format!(
                r##"{}
ex:target ga:metadataRow ex:version-link, ex:latest-link, ex:edition-links .
ex:version-link a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:latest-link a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:edition-links a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ;
    dct:title "Editions:" ; ga:body """{links}""" .
"##,
                include_str!("../examples/specification/project.ttl")
            ),
        )?;
    }
    let output = tempfile::tempdir()?;
    let report = showcases::verify_gallery(
        Dataset::memory()?,
        &root.path().join("examples"),
        "urn:grapharch:docs",
        output.path(),
    )
    .await?;
    assert!(!report.succeeded);
    let error = report.cases[0].error.as_deref().unwrap_or_default();
    assert!(
        error.contains(
            "specification/index.html: ../pdf/omg/document.pdf \
             (pdf/omg/document.pdf is not published by this showcase)"
        ),
        "a link to a PDF the showcase does not publish fails it: {error}"
    );
    assert!(
        report.cases[1].succeeded,
        "relative links to the showcase's own files pass: {:?}",
        report.cases[1].error
    );
    Ok(())
}

#[tokio::test]
async fn definitions_place_the_documentation_graph_inside_a_publication()
-> Result<()> {
    use grapharch::publication::{dataset::Dataset, showcases};
    let root = tempfile::tempdir()?;
    registry(
        root.path(),
        r#"
<urn:case:first> a ga:Showcase;ga:slug "first";ga:showcaseRunner ga:SpecificationShowcaseRunner;ga:source "unplaced.ttl" .
<urn:case:second> a ga:Showcase;ga:slug "second";ga:showcaseRunner ga:SpecificationShowcaseRunner;ga:source "placed.ttl" .
"#,
    )?;
    for (name, placement) in [
        ("unplaced.ttl", ""),
        (
            "placed.ttl",
            r#"ex:target ga:documentationGraphPath "graph/documentation.nq" ."#,
        ),
    ] {
        std::fs::write(
            root.path().join("examples").join(name),
            format!(
                r##"{}
ex:target ga:metadataRow ex:version-link, ex:latest-link, ex:downloads .
ex:version-link a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:latest-link a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:downloads a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ;
    dct:title "Downloads:" ; ga:body """<a href="graph/documentation.nq">Documentation graph</a>""" .
{placement}
"##,
                include_str!("../examples/specification/project.ttl")
            ),
        )?;
    }
    let output = tempfile::tempdir()?;
    let report = showcases::verify_gallery(
        Dataset::memory()?,
        &root.path().join("examples"),
        "urn:grapharch:docs",
        output.path(),
    )
    .await?;
    let error = report.cases[0].error.as_deref().unwrap_or_default();
    assert!(
        error.contains("specification/graph/documentation.nq is not published"),
        "an unplaced graph link fails: {error}"
    );
    assert!(
        report.cases[1].succeeded,
        "the declared graph placement publishes the linked file: {:?}",
        report.cases[1].error
    );
    let placed = output
        .path()
        .join("artifacts/second/specification/graph/documentation.nq");
    assert_eq!(
        std::fs::read(&placed)?,
        std::fs::read(output.path().join("artifacts/second/documentation.nq"))?,
        "the placed graph is the showcase's documentation graph"
    );
    Ok(())
}
