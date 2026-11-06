#[path = "support/definitions.rs"]
mod definitions;
#[path = "support/dprod_repository.rs"]
mod dprod_repository;
#[path = "support/family_repository.rs"]
mod family_repository;

use {
    anyhow::{Result, ensure},
    std::path::Path,
    tokio::process::Command,
};

async fn build(
    repository: &str,
    revision: &str,
    root: &Path,
    definitions: &str,
) -> Result<std::process::Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args([
            "publication",
            "build-dprod",
            "--json",
            "--git-repository",
            repository,
            "--git-ref",
            revision,
            "--definitions",
            definitions,
            "--generator-revision",
            "1111111111111111111111111111111111111111",
            "--git-cache",
        ])
        .arg(root.join("cache"))
        .arg("--output-root")
        .arg(root.join("published"))
        .output()
        .await?)
}

#[tokio::test]
async fn exact_repository_build_has_no_gallery_dependency_and_preserves_source_identity()
-> Result<()> {
    let source = tempfile::tempdir()?;
    family_repository::copy_tree(Path::new("examples/dprod"), source.path())?;
    let remote = tempfile::tempdir()?;
    dprod_repository::configure(source.path(), remote.path())?;
    let config: serde_yaml::Value = serde_yaml::from_str(
        &tokio::fs::read_to_string(source.path().join("live.yaml")).await?,
    )?;
    let repository = config["repository"].as_str().unwrap();
    let revision = config["ref"].as_str().unwrap();
    let output = tempfile::tempdir()?;
    let result = build(
        repository,
        revision,
        output.path(),
        "publication/standards.ttl",
    )
    .await?;
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(manifest["source_commit"], revision);
    assert_eq!(manifest["definitions_commit"], revision);
    assert_eq!(
        manifest["generator_revision"],
        "1111111111111111111111111111111111111111"
    );
    assert_eq!(
        manifest["publications"].as_array().unwrap().len(),
        3
    );
    assert!(output.path().join("published/documentation.nq").is_file());
    assert!(!output.path().join("published/gallery.html").exists());
    let html = tokio::fs::read_to_string(
        output.path().join("published/specification/index.html"),
    )
    .await?;
    assert!(html.contains("SOURCE OWNED FORMAL SCOPE"));
    let missing = tempfile::tempdir()?;
    let result = build(
        repository,
        revision,
        missing.path(),
        "publication/absent.ttl",
    )
    .await?;
    assert!(!result.status.success());
    assert!(
        !missing.path().join("published").exists(),
        "failed collection must not promote a partial publication"
    );
    Ok(())
}

#[tokio::test]
async fn explicit_publication_date_does_not_rewrite_old_native_rdf()
-> Result<()> {
    let source = tempfile::tempdir()?;
    family_repository::copy_tree(Path::new("examples/dprod"), source.path())?;
    let remote = tempfile::tempdir()?;
    dprod_repository::configure(source.path(), remote.path())?;
    let checkout = remote.path().join("checkout");
    let ontology = checkout.join("ontology/dprod/dprod-ontology.ttl");
    let content = definitions::remove_property(
        &tokio::fs::read_to_string(&ontology).await?,
        "https://www.omg.org/spec/DPROD/dprod/",
        "http://purl.org/dc/terms/issued",
    )?;
    tokio::fs::write(&ontology, &content).await?;
    let config = checkout.join("publication/standards.ttl");
    let mut settings = tokio::fs::read_to_string(&config).await?;
    settings.push_str("\n<urn:dprod-showcase:specification> <https://ekgf.org/ontology/grapharch#publicationDate> \"2025-01-02\"^^<http://www.w3.org/2001/XMLSchema#date> .\n<urn:dprod-showcase:pdf> <https://ekgf.org/ontology/grapharch#publicationDate> \"2025-01-02\"^^<http://www.w3.org/2001/XMLSchema#date> .\n");
    tokio::fs::write(&config, settings).await?;
    family_repository::git(&checkout, &["add", "."])?;
    family_repository::git(&checkout, &[
        "commit",
        "-m",
        "Old source with explicit publication date",
    ])?;
    let revision = family_repository::git(&checkout, &["rev-parse", "HEAD"])?;
    let repository =
        url::Url::from_directory_path(remote.path().join("remote.git"))
            .unwrap()
            .to_string();
    family_repository::git(&checkout, &[
        "push",
        &repository,
        "HEAD:refs/heads/main",
    ])?;
    let output = tempfile::tempdir()?;
    let result = build(
        &repository,
        &revision,
        output.path(),
        "publication/standards.ttl",
    )
    .await?;
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        tokio::fs::read_to_string(ontology).await?,
        content
    );
    assert!(
        tokio::fs::read_to_string(
            output.path().join("published/specification/index.html")
        )
        .await?
        .contains("2025-01-02")
    );
    Ok(())
}
