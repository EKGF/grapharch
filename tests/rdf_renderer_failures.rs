//! Invalid RDF renderer targets and content redefining RDF formats fail.
#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/rdf_targets.rs"]
mod rdf_targets;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

use {
    anyhow::Result,
    rdf_targets::{TARGETS, build},
    revision_fixture::RevisionFixture,
    std::path::Path,
};

#[tokio::test]
async fn invalid_rdf_targets_fail_the_build() -> Result<()> {
    let selected = [
        "--target",
        "urn:dprod-showcase:rdf-source",
        "--target",
        "urn:dprod-showcase:rdf-all",
    ];
    for (definitions, message) in [
        (
            TARGETS.replace(
                "ga:rdfSource ex:collector:ontology ; ga:rdfFormat \
                 ga:SourceTextFormat",
                "ga:rdfSource ex:collector:ontology, ex:collector:shapes ; \
                 ga:rdfFormat ga:SourceTextFormat",
            ),
            "needs exactly one ga:rdfSource",
        ),
        (
            TARGETS.replace(
                "ga:rdfFormat ga:SourceTextFormat .",
                "ga:rdfFormat ga:SourceTextFormat, ga:TurtleFormat .",
            ),
            "publishes dprod.ttl for more than one format",
        ),
        (
            TARGETS.replace(
                "ga:slug \"dprod\" ;",
                "ga:slug \"dprod\" ; ga:scope ex:scope ;",
            ),
            "declares scope",
        ),
        (
            TARGETS.replace("ga:slug \"dprod\" ;", ""),
            "requires a ga:slug",
        ),
        (
            TARGETS.replace("ga:slug \"dprod\"", "ga:slug \"Dprod\""),
            "invalid slug",
        ),
        (
            TARGETS.replace(
                "ga:rdfSource ex:collector:ontology ;",
                "ga:rdfSource ex:collector:abstract ;",
            ),
            "must be a ga:RdfFileCollector use",
        ),
        (
            TARGETS.replace(
                "ga:rdfFormat ga:SourceTextFormat",
                "ga:rdfFormat ga:N3Format",
            ),
            "references undefined RdfFormat",
        ),
        (
            TARGETS.replace(" ; ga:negotiatedHtml ex:specification", ""),
            "ga:negotiatedPath and ga:negotiatedHtml together",
        ),
        (
            TARGETS.replace(
                "ga:negotiatedHtml ex:specification",
                "ga:negotiatedHtml ex:pdf",
            ),
            "publishes no entry document",
        ),
        (
            TARGETS.replace(
                "ga:negotiatedPath \"ontology\"",
                "ga:negotiatedPath \"../ontology\"",
            ),
            "invalid ga:negotiatedPath",
        ),
        (
            TARGETS.replace("ga:slug \"dprod-all\"", "ga:slug \"dprod\""),
            "dprod.ttl is already published by target",
        ),
        (
            TARGETS.replace(
                "ga:destination \"rdf\" ; ga:slug \"dprod\"",
                "ga:destination \"specification\" ; ga:slug \"dprod\"",
            ),
            "share the output destination",
        ),
        (
            format!(
                "{TARGETS}\nex:specification ga:rdfFormat ga:TurtleFormat ."
            ),
            "which only a ga:RdfRenderer target uses",
        ),
    ] {
        let (output, _temp) = build(&definitions, &selected).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{message}: accepted");
        assert!(stderr.contains(message), "{message}: {stderr}");
    }
    Ok(())
}

#[tokio::test]
async fn content_cannot_redefine_an_rdf_format() -> Result<()> {
    let mut fixture = RevisionFixture::new(Path::new("examples/dprod"))?;
    fixture.edit_definitions(|definitions| {
        Ok(format!("{definitions}\n{TARGETS}"))
    })?;
    fixture.edit_content("ontology.ttl", |text| {
        Ok(format!(
            "{text}\n<https://ekgf.org/ontology/grapharch#TurtleFormat> \
             <https://ekgf.org/ontology/grapharch#mediaType> \"text/plain\" .\n"
        ))
    })?;
    let output = tempfile::tempdir()?;
    let result = fixture
        .build(&output.path().join("published"), &[
            "--target",
            "urn:dprod-showcase:rdf-all",
        ])
        .await?;
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(
        stderr.contains(
            "only the controlling definitions may define publications"
        ),
        "{stderr}"
    );
    Ok(())
}
