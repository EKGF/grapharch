use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family::{self, FamilySource},
    },
    oxrdf::{GraphName, NamedNode, Quad},
};
struct Fixture {
    root:        tempfile::TempDir,
    definitions: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Result<Self> {
        let fixture = Self {
            root:        tempfile::tempdir()?,
            definitions: tempfile::tempdir()?,
        };
        for name in ["core", "extra"] {
            std::fs::write(
                fixture.root.path().join(format!("{name}.ttl")),
                format!(
                    r#"@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix sh:<http://www.w3.org/ns/shacl#> .
<https://example.org/{name}#> a owl:Ontology;rdfs:label "{name} ontology"@en;sh:declare [sh:prefix "{name}";sh:namespace "https://example.org/{name}#"] .
<https://example.org/{name}#Term> a owl:Class;rdfs:label "{name} term"@en;rdfs:comment "A supplied definition."@en ."#
                ),
            )?;
        }
        fixture.configure(true)?;
        Ok(fixture)
    }

    fn configure(&self, extra: bool) -> Result<()> {
        let members = if extra {
            "<https://example.org/extra#> gov:inFamily <urn:family> ."
        } else {
            ""
        };
        std::fs::write(
            self.definitions.path().join("project.ttl"),
            format!(
                r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix gov:<https://ekgf.org/ontology/artifact-governance#> . @prefix pub:<https://ekgf.org/ontology/artifact-publication#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:project> a ga:Project;ga:target <urn:target> . <urn:scope> a ga:Scope . <urn:profile> a ga:Profile;pub:publishesFamily <urn:family> .
<urn:family> a gov:Family;rdfs:label "Example family"@en . <https://example.org/core#> gov:inFamily <urn:family> . {members}
<urn:target> a ga:Target;dct:title "Example reference";dct:description "A family reference.";rdfs:comment "A test draft.";ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer "specification";ga:destination "specification";ga:publicationStatus "Draft";ga:publicationDate "2026-10-25"^^<http://www.w3.org/2001/XMLSchema#date>;ga:thisVersion <https://example.org/spec/>;ga:latestVersion <https://example.org/spec/>;ga:license <https://creativecommons.org/publicdomain/zero/1.0/>;ga:publisher <urn:authority> .
<urn:authority> a ga:Authority;rdfs:label "Example Authority";ga:shortName "Example" ."#
            ),
        )?;
        Ok(())
    }

    async fn build(
        &self,
        dataset: Dataset,
    ) -> Result<ontology_family::CollectedFamily> {
        ontology_family::collect_from(
            dataset,
            FamilySource::Directory(self.root.path()),
            &self.definitions.path().join("project.ttl"),
            "urn:project",
            "urn:target",
            2,
        )
        .await
    }
}
#[tokio::test]
async fn unchanged_family_builds_reuse_revision_and_plan_in_a_shared_store()
-> Result<()> {
    let f = Fixture::new()?;
    let dataset = Dataset::memory()?;
    let a = f.build(dataset.clone()).await?;
    let b = f.build(dataset).await?;
    assert_eq!(
        a.plan.id, b.plan.id,
        "generated inputs must not recursively depend on their own previous \
         outputs"
    );
    assert_eq!(a.plan.snapshot, b.plan.snapshot);
    Ok(())
}
#[tokio::test]
async fn removed_owned_sources_leave_current_views_but_historical_snapshots_survive()
-> Result<()> {
    let f = Fixture::new()?;
    let dataset = Dataset::memory()?;
    f.build(dataset.clone()).await?;
    let old = dataset.snapshot_for("urn:project").await?;
    std::fs::remove_file(f.root.path().join("extra.ttl"))?;
    f.configure(false)?;
    f.build(dataset.clone()).await?;
    let next = dataset.snapshot_for("urn:project").await?;
    let quad = Quad::new(
        NamedNode::new("https://example.org/extra#Term")?,
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        NamedNode::new("http://www.w3.org/2002/07/owl#Class")?,
        GraphName::DefaultGraph,
    );
    assert!(dataset.contains(&old, quad.clone()).await?);
    assert!(
        !dataset.contains(&next, quad).await?,
        "removed owned input must not remain active"
    );
    Ok(())
}
#[tokio::test]
async fn failed_discovery_blocks_new_assembly_without_destroying_the_last_revision()
-> Result<()> {
    let f = Fixture::new()?;
    let dataset = Dataset::memory()?;
    f.build(dataset.clone()).await?;
    let previous =
        grapharch::publication::assembly::Assembler::new(dataset.clone())
            .current("urn:project")
            .await?;
    let result = ontology_family::collect_from(
        dataset.clone(),
        FamilySource::Directory(&f.root.path().join("missing")),
        &f.definitions.path().join("project.ttl"),
        "urn:project",
        "urn:target",
        2,
    )
    .await;
    assert!(result.is_err());
    assert!(
        dataset.ensure_ready("urn:project").await.is_err(),
        "a failed discovery must register an incomplete/failed input"
    );
    assert_eq!(
        grapharch::publication::assembly::Assembler::new(dataset)
            .current("urn:project")
            .await?,
        previous
    );
    Ok(())
}
#[path = "sparql_support/mod.rs"]
mod support;
async fn publish_restore(dataset: Dataset) -> Result<()> {
    let f = Fixture::new()?;
    let built = f.build(dataset.clone()).await?;
    let old_sources = dataset.snapshot_for("urn:project").await?;
    let repeated = f.build(dataset.clone()).await?;
    assert_eq!(built.plan.id, repeated.plan.id);
    std::fs::remove_file(f.root.path().join("extra.ttl"))?;
    f.configure(false)?;
    let changed = f.build(dataset.clone()).await?;
    assert_ne!(built.plan.snapshot, changed.plan.snapshot);
    let removed = Quad::new(
        NamedNode::new("https://example.org/extra#Term")?,
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        NamedNode::new("http://www.w3.org/2002/07/owl#Class")?,
        GraphName::DefaultGraph,
    );
    assert!(dataset.contains(&old_sources, removed.clone()).await?);
    assert!(
        !dataset
            .contains(
                &dataset.snapshot_for("urn:project").await?,
                removed
            )
            .await?
    );
    let output = tempfile::tempdir()?;
    let archive = output.path().join("archive.nq");
    dataset.export(archive.clone()).await?;
    drop(f);
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let outcome = grapharch::publication::render::Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?;
    let grapharch::publication::render::PublishOutcome::Published(published) =
        outcome
    else {
        panic!("expected publication")
    };
    let html = std::fs::read_to_string(published.directory.join("index.html"))?;
    assert!(html.contains("core ontology"));
    assert!(html.contains("A supplied definition."));
    Ok(())
}
#[tokio::test]
async fn external_family_publishes_from_restored_pinned_kg() -> Result<()> {
    let server = support::Server::new(true)?;
    publish_restore(Dataset::sparql(server.config()).await?).await?;
    assert_eq!(server.transaction_count(), 0);
    Ok(())
}
#[tokio::test]
#[ignore = "requires an explicitly configured local RDF4J NativeStore \
            repository"]
async fn native_rdf4j_family_publishes_from_restored_pinned_kg() -> Result<()> {
    let endpoint = std::env::var("GRAPHARCH_TEST_RDF4J_URL")?;
    let url = url::Url::parse(&endpoint)?;
    anyhow::ensure!(
        matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]")
        ),
        "integration test refuses non-loopback services"
    );
    let config = serde_yaml::from_str(&format!(
        "query_endpoint: {endpoint}\ntransactions_endpoint: \
         {endpoint}/transactions\n"
    ))?;
    publish_restore(Dataset::sparql(config).await?).await
}
#[tokio::test]
async fn family_cli_json_is_machine_readable_when_logging_is_enabled()
-> Result<()> {
    let f = Fixture::new()?;
    let output = tempfile::tempdir()?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["-vv", "publication", "build-family", "--source"])
        .arg(f.root.path())
        .arg("--definitions")
        .arg(f.definitions.path().join("project.ttl"))
        .args([
            "--project",
            "urn:project",
            "--target",
            "urn:target",
            "--output-root",
        ])
        .arg(output.path().join("published family"))
        .arg("--json")
        .env("RUST_LOG", "info")
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(value["schema_version"], 1);
    assert!(
        value["plan"]
            .as_str()
            .unwrap()
            .starts_with("urn:grapharch:publication-plan:")
    );
    for key in ["publication_path", "pdf_path", "kg_path"] {
        let path = std::path::Path::new(value[key].as_str().unwrap());
        assert!(path.is_absolute());
        assert!(path.exists(), "missing {key}");
    }
    assert!(
        std::path::Path::new(value["publication_path"].as_str().unwrap())
            .join("index.html")
            .is_file()
    );
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("family sources collected")
    );
    Ok(())
}
#[tokio::test]
async fn site_cli_json_reports_the_actual_published_directory() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("sources");
    std::fs::create_dir(&source)?;
    std::fs::write(
        source.join("README.md"),
        "---\ntitle: Welcome\npermalink: /\n---\n# Welcome\nA minimal \
         companion website.\n",
    )?;
    let definitions = source.join("project.ttl");
    std::fs::write(
        &definitions,
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:site> a ga:Project;dct:title "Minimal docs";ga:scope <urn:scope>;ga:target <urn:website> .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile .
<urn:website> a ga:Target;dct:title "Minimal docs";ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer "website";ga:destination "unusual/destination" ."#,
    )?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["-vv", "publication", "build-site", "--source"])
        .arg(source)
        .arg("--definitions")
        .arg(definitions)
        .args([
            "--project",
            "urn:site",
            "--target",
            "urn:website",
            "--output-root",
        ])
        .arg(root.path().join("output"))
        .arg("--json")
        .env("RUST_LOG", "info")
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(value["schema_version"], 1);
    assert!(value["pdf_path"].is_null());
    let publication =
        std::path::Path::new(value["publication_path"].as_str().unwrap());
    assert!(publication.ends_with("unusual/destination"));
    assert!(publication.join("index.html").exists());
    assert!(std::path::Path::new(value["kg_path"].as_str().unwrap()).is_file());
    Ok(())
}
#[tokio::test]
async fn failed_json_builds_emit_no_success_object() -> Result<()> {
    let root = tempfile::tempdir()?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "build-site", "--source"])
        .arg(root.path().join("missing"))
        .arg("--definitions")
        .arg(root.path().join("missing.ttl"))
        .args([
            "--project",
            "urn:site",
            "--target",
            "urn:website",
            "--output-root",
        ])
        .arg(root.path().join("output"))
        .arg("--json")
        .env("RUST_LOG", "info")
        .output()
        .await?;
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.is_empty());
    Ok(())
}
