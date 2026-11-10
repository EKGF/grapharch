use {anyhow::Result, tokio::process::Command};

const ONTOLOGY: &str = r#"@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/pets> a owl:Ontology; rdfs:label "Pets" .
<https://example.org/pets#Pet> a owl:Class; rdfs:label "Pet";
 rdfs:comment "An animal kept for company." .
"#;

#[tokio::test]
async fn ontology_file_builds_without_configuration() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("pets.ttl");
    tokio::fs::write(&source, ONTOLOGY).await?;
    let result = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(&source)
        .arg("--output")
        .arg(root.path().join("output"))
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html =
        tokio::fs::read_to_string(root.path().join("output/site/index.html"))
            .await?;
    assert!(html.contains("Pets"));
    assert!(html.contains("An animal kept for company."));
    assert!(root.path().join("output/documentation.nq").exists());
    assert!(!html.contains("Editor's Draft"));
    Ok(())
}

#[tokio::test]
async fn plain_markdown_directory_has_titles_links_and_safe_discovery()
-> Result<()> {
    let root = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("README.md"),
        "# Pet project\n\nRead the [guide](guide.md).\n",
    )
    .await?;
    tokio::fs::write(
        root.path().join("guide.md"),
        "# Care guide\n\nGive pets fresh water.\n",
    )
    .await?;
    tokio::fs::create_dir(root.path().join("node_modules")).await?;
    tokio::fs::write(
        root.path().join("node_modules/broken.ttl"),
        "not RDF",
    )
    .await?;
    let result = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .current_dir(root.path())
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = root.path().join("output/grapharch/site");
    let html = tokio::fs::read_to_string(output.join("index.html")).await?;
    assert!(html.contains("Pet project"));
    assert!(output.join("guide/index.html").exists());
    let repeat = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .current_dir(root.path())
        .output()
        .await?;
    assert!(
        repeat.status.success(),
        "{}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    Ok(())
}

#[tokio::test]
async fn invalid_config_is_not_replaced_with_defaults() -> Result<()> {
    let root = tempfile::tempdir()?;
    tokio::fs::write(root.path().join("README.md"), "# Project\n").await?;
    tokio::fs::write(
        root.path().join("grapharch.ttl"),
        "invalid config",
    )
    .await?;
    let result = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(root.path())
        .output()
        .await?;
    assert!(!result.status.success());
    assert!(
        !String::from_utf8_lossy(&result.stderr)
            .contains("unrecognized subcommand")
    );
    Ok(())
}

#[tokio::test]
async fn shared_rdf_subjects_render_once_and_survive_source_removal()
-> Result<()> {
    use grapharch::publication::{
        dataset::Dataset,
        quick_build::BuildRequest,
        render::{PublishOutcome, Publisher},
    };
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("README.md"),
        "# Pet project\n\nRead the [reference](pets.ttl).\n",
    )
    .await?;
    tokio::fs::write(source.path().join("pets.ttl"), ONTOLOGY).await?;
    tokio::fs::write(source.path().join("zzduplicate.ttl"), "<https://example.org/pets#Pet> a <http://www.w3.org/2002/07/owl#Class>; <http://www.w3.org/2000/01/rdf-schema#comment> \"Care matters.\" .").await?;
    let request = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().join("initial"),
    };
    let (dataset, plan) = request.collect().await?;
    let PublishOutcome::Published(original) = Publisher::new(dataset.clone())
        .render(&plan, &request.output)
        .await?
    else {
        panic!("published")
    };
    let before = tokio::fs::read(original.directory.join("index.html")).await?;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, &output.path().join("restored"))
        .await?
    else {
        panic!("published")
    };
    assert_eq!(
        before,
        tokio::fs::read(publication.directory.join("index.html")).await?
    );
    assert!(publication.directory.join("provenance.html").exists());
    assert!(
        publication
            .directory
            .join("reference/zzduplicatettl/index.html")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn configured_website_keeps_explicit_destination_and_title() -> Result<()>
{
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("project.ttl"), r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Chosen title"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "chosen" .
<urn:profile> a ga:Profile .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Explicit home\n---\n# Hello\n",
    )
    .await?;
    let result = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(source.path())
        .arg("--output")
        .arg(source.path().join("output"))
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(source.path().join("output/chosen/index.html").exists());
    Ok(())
}

#[tokio::test]
async fn explicit_config_is_validated_before_unconfigured_discovery_limits()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("grapharch.ttl"),
        "invalid config",
    )
    .await?;
    for index in 0..257 {
        tokio::fs::write(
            source.path().join(format!("page{index}.md")),
            "# Page",
        )
        .await?;
    }
    let result = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(source.path())
        .output()
        .await?;
    assert!(!result.status.success());
    assert!(
        !String::from_utf8_lossy(&result.stderr)
            .contains("exceeds 256 sources")
    );
    Ok(())
}
