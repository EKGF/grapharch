use tokio::process::Command;

#[tokio::test]
async fn independent_commands_collect_reopen_and_export_the_same_dataset()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("intro.md");
    let store = dir.path().join("store");
    let export = dir.path().join("dataset.nq");
    tokio::fs::write(
        &source,
        "# Introduction\n\nDocumentation from a CLI invocation.\n",
    )
    .await?;
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "collect", "--store"])
        .arg(&store)
        .args([
            "--project",
            "urn:project",
            "--collector",
            "urn:collector",
            "--kind",
            "markdown",
            "--source",
        ])
        .arg(&source)
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "snapshot", "--store"])
        .arg(&store)
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)?
            .trim()
            .starts_with("urn:grapharch:snapshot:")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "export", "--store"])
        .arg(&store)
        .arg("--output")
        .arg(&export)
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(tokio::fs::metadata(export).await?.len() > 0);
    Ok(())
}

#[tokio::test]
async fn independent_assembly_and_plan_commands_use_a_validated_revision()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = dir.path().join("store");
    let project = "urn:example:ontology-project";
    let collect = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "collect", "--store"])
        .arg(&store)
        .args([
            "--project",
            project,
            "--collector",
            "urn:collector:definitions",
            "--kind",
            "rdf",
            "--source",
            "tests/fixtures/publication/projects.ttl",
        ])
        .output()
        .await?;
    assert!(
        collect.status.success(),
        "{}",
        String::from_utf8_lossy(&collect.stderr)
    );
    let assembled = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "assemble", "--store"])
        .arg(&store)
        .args(["--project", project])
        .output()
        .await?;
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );
    let revision = String::from_utf8(assembled.stdout)?;
    let planned = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "plan", "--store"])
        .arg(&store)
        .args(["--project", project, "--revision", revision.trim(), "--all"])
        .output()
        .await?;
    assert!(
        planned.status.success(),
        "{}",
        String::from_utf8_lossy(&planned.stderr)
    );
    let result = String::from_utf8(planned.stdout)?;
    assert_eq!(
        result
            .lines()
            .filter(|line| line.starts_with("plan="))
            .count(),
        3
    );
    assert_eq!(
        result
            .lines()
            .filter(|line| line.starts_with("snapshot="))
            .count(),
        3
    );
    Ok(())
}

#[tokio::test]
async fn rendering_restarts_from_the_persisted_plan_after_source_removal()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = dir.path().join("store");
    let source = dir.path().join("project.ttl");
    let root = dir.path().join("publications");
    let project = "urn:example:ontology-project";
    tokio::fs::write(
        &source,
        include_str!("fixtures/publication/projects.ttl"),
    )
    .await?;
    let collect = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "collect", "--store"])
        .arg(&store)
        .args([
            "--project",
            project,
            "--collector",
            "urn:collector:definitions",
            "--kind",
            "rdf",
            "--source",
        ])
        .arg(&source)
        .output()
        .await?;
    assert!(
        collect.status.success(),
        "{}",
        String::from_utf8_lossy(&collect.stderr)
    );
    let assemble = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "assemble", "--store"])
        .arg(&store)
        .args(["--project", project])
        .output()
        .await?;
    assert!(assemble.status.success());
    let revision = String::from_utf8(assemble.stdout)?;
    let plan = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "plan", "--store"])
        .arg(&store)
        .args([
            "--project",
            project,
            "--revision",
            revision.trim(),
            "--target",
            "urn:example:spec",
        ])
        .output()
        .await?;
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan = String::from_utf8(plan.stdout)?;
    let id = plan
        .lines()
        .find_map(|line| line.strip_prefix("plan="))
        .ok_or_else(|| anyhow::anyhow!("missing plan"))?;
    let snapshot = plan
        .lines()
        .find_map(|line| line.strip_prefix("snapshot="))
        .ok_or_else(|| anyhow::anyhow!("missing plan snapshot"))?;
    tokio::fs::remove_file(source).await?;
    let render = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "render", "--store"])
        .arg(&store)
        .args(["--plan", id, "--snapshot", snapshot, "--output-root"])
        .arg(&root)
        .output()
        .await?;
    assert!(
        render.status.success(),
        "{}",
        String::from_utf8_lossy(&render.stderr)
    );
    assert!(
        tokio::fs::read_to_string(root.join("build/spec/index.html"))
            .await?
            .contains("Shared introduction.")
    );
    Ok(())
}

#[tokio::test]
async fn w3c_collection_requires_its_original_publication_url_before_opening_store()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = dir.path().join("store");
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "collect", "--store"])
        .arg(&store)
        .args([
            "--project",
            "urn:project:w3c",
            "--collector",
            "urn:collector:w3c",
            "--kind",
            "w3c-specification",
            "--source",
            "examples/w3c-rdf12-n-triples/source.html",
        ])
        .env("RUST_LOG", "off")
        .output()
        .await?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source-url"));
    assert!(output.stdout.is_empty());
    assert!(!store.exists());
    Ok(())
}

#[test]
fn execution_policy_flags_are_global_and_mutually_exclusive() {
    use {clap::Parser, grapharch::cli::Cli};
    let args = [
        "grapharch",
        "publication",
        "validate",
        "--definitions",
        "unused.ttl",
        "--keep-going",
    ];
    let cli = Cli::try_parse_from(args).unwrap();
    assert!(cli.keep_going);
    let conflict = Cli::try_parse_from([
        "grapharch",
        "--fail-fast",
        "--keep-going",
        "publication",
        "validate",
        "--definitions",
        "unused.ttl",
    ]);
    assert!(
        conflict
            .err()
            .unwrap()
            .to_string()
            .contains("cannot be used with")
    );
}
