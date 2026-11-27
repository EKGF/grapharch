//! A profile can disable the OWL class-diagram fragment renderer, so a family
//! build documents its classes without a local PlantUML or Java installation.
use {
    anyhow::Result,
    std::{path::Path, process::Output},
    tokio::process::Command,
};

const ONTOLOGY: &str = r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Diagram family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>;
 rdfs:label "Diagram ontology"@en;
 sh:declare [sh:prefix "example"; sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Parent> a owl:Class; rdfs:label "Parent"@en;
 rdfs:comment "A documented superclass."@en .
<https://example.org/ontology#Thing> a owl:Class; rdfs:label "Thing"@en;
 rdfs:comment "A documented class with a superclass."@en;
 rdfs:subClassOf <https://example.org/ontology#Parent> .
"#;

fn definitions(profile_extra: &str) -> String {
    format!(
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<urn:example:project> a ga:Project; ga:target <urn:example:spec> .
<urn:example:scope> a ga:Scope .
<urn:example:profile> a ga:Profile, pub:PublicationProfile;
 pub:publishesFamily <urn:example:family> {profile_extra}.
<urn:example:spec> a ga:Target; dct:title "Diagram family";
 dct:description "One family published as specification pages.";
 rdfs:comment "Documentation preview.";
 ga:scope <urn:example:scope>; ga:profile <urn:example:profile>;
 ga:renderer ga:SpecificationPagesRenderer; ga:documentType ga:SpecificationDocument;
 ga:slug "spec"; ga:destination "spec/main";
 ga:thisVersion <https://example.org/spec/main/>; ga:latestVersion <https://example.org/spec/main/>;
 ga:license <https://creativecommons.org/publicdomain/zero/1.0/>; ga:publicationStatus "Editor's Draft";
 ga:publicationDate "2026-11-12"^^<http://www.w3.org/2001/XMLSchema#date>; ga:publisher <urn:example:authority> .
<urn:example:authority> a <https://ekgf.org/ontology/artifact-governance#Authority>;
 rdfs:label "Example authority"@en; ga:shortName "Example"; ga:slug "example" .
"#
    )
}

/// Build the family; `tools` replaces `PATH` and removes `JAVA_HOME`, so the
/// build sees no PlantUML launcher and no Java.
async fn build(
    root: &Path,
    profile_extra: &str,
    tools: Option<&Path>,
) -> Result<Output> {
    let source = root.join("source");
    tokio::fs::create_dir_all(&source).await?;
    tokio::fs::write(source.join("ontology.ttl"), ONTOLOGY).await?;
    let definitions_path = root.join("grapharch.ttl");
    tokio::fs::write(&definitions_path, definitions(profile_extra)).await?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_grapharch"));
    if let Some(tools) = tools {
        command.env("PATH", tools).env_remove("JAVA_HOME");
    }
    Ok(command
        .args(["publication", "build-family", "--source"])
        .arg(&source)
        .arg("--definitions")
        .arg(&definitions_path)
        .args([
            "--project",
            "urn:example:project",
            "--target",
            "urn:example:spec",
        ])
        .arg("--output-root")
        .arg(root.join("output"))
        .output()
        .await?)
}

async fn retained_graph(root: &Path) -> Result<String> {
    Ok(tokio::fs::read_to_string(root.join("output/documentation.nq")).await?)
}

const DISABLED: &str = "; ga:disableRenderer ga:OwlClassDiagramRenderer ";

#[tokio::test]
async fn disabled_class_diagram_renderer_needs_no_plantuml_or_java()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let tools = root.path().join("no-layout-tools");
    tokio::fs::create_dir(&tools).await?;
    let built = build(root.path(), DISABLED, Some(&tools)).await?;
    assert!(
        built.status.success(),
        "a profile without class diagrams needs neither PlantUML nor Java: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let graph = retained_graph(root.path()).await?;
    assert!(graph.contains("A documented class with a superclass."));
    assert!(
        !graph.contains("grapharch-class-diagram"),
        "no class-diagram fragment is generated"
    );
    Ok(())
}

#[tokio::test]
async fn class_diagrams_are_rendered_by_default() -> Result<()> {
    let root = tempfile::tempdir()?;
    let built = build(root.path(), "", None).await?;
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(
        retained_graph(root.path())
            .await?
            .contains("grapharch-class-diagram"),
        "the default profile draws every OWL class"
    );
    let missing = tempfile::tempdir()?;
    let tools = missing.path().join("no-layout-tools");
    tokio::fs::create_dir(&tools).await?;
    let built = build(missing.path(), "", Some(&tools)).await?;
    assert!(
        !built.status.success(),
        "the default profile still requires the local renderer"
    );
    assert!(
        String::from_utf8_lossy(&built.stderr).contains("PlantUML"),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    Ok(())
}

#[tokio::test]
async fn only_fragment_renderers_can_be_disabled() -> Result<()> {
    for value in ["ga:WebsiteRenderer", "\"OwlClassDiagramRenderer\""] {
        let root = tempfile::tempdir()?;
        let built = build(
            root.path(),
            &format!("; ga:disableRenderer {value} "),
            None,
        )
        .await?;
        assert!(!built.status.success(), "accepted {value}");
        assert!(
            String::from_utf8_lossy(&built.stderr).contains("disableRenderer"),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
    }
    Ok(())
}
