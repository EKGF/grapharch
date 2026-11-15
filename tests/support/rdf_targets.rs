//! RDF renderer targets added to the DPROD fixture definitions.
use {
    super::revision_fixture::RevisionFixture,
    anyhow::Result,
    std::path::Path,
};

/// RDF targets of the DPROD fixture: the authoritative ontology file as
/// collected, and the merged ontology and shapes in every official format.
pub const TARGETS: &str = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:dprod-showcase:> .
ex:project ga:target ex:rdf-source, ex:rdf-all .
ex:rdf-source a ga:Target ; ga:renderer ga:RdfRenderer ;
    ga:destination "rdf" ; ga:slug "dprod" ;
    ga:rdfSource ex:collector:ontology ; ga:rdfFormat ga:SourceTextFormat .
ex:rdf-all a ga:Target ; ga:renderer ga:RdfRenderer ;
    ga:destination "rdf" ; ga:slug "dprod-all" ;
    ga:rdfSource ex:collector:ontology, ex:collector:shapes ;
    ga:rdfFormat ga:TurtleFormat, ga:NTriplesFormat, ga:NQuadsFormat,
        ga:TriGFormat, ga:JsonLdFormat, ga:RdfXmlFormat ;
    ga:negotiatedPath "ontology" ; ga:negotiatedHtml ex:specification .
"#;
/// Build the fixture with `extra` definitions and the `selection` arguments.
pub async fn build(
    extra: &str,
    selection: &[&str],
) -> Result<(std::process::Output, tempfile::TempDir)> {
    let mut fixture = RevisionFixture::new(Path::new("examples/dprod"))?;
    fixture.edit_definitions(|definitions| {
        Ok(format!("{definitions}\n{extra}"))
    })?;
    let output = tempfile::tempdir()?;
    let result = fixture
        .build(&output.path().join("published"), selection)
        .await?;
    Ok((result, output))
}
