#[tokio::test]
async fn family_cli_supports_local_and_github_repository_sources()
-> anyhow::Result<()> {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "build-family", "--help"])
        .output()
        .await?;
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout)?;
    assert!(
        help.contains("--local-repository"),
        "local repo must be a first-class source"
    );
    assert!(
        help.contains("--github"),
        "GitHub repo must be a first-class source"
    );
    assert!(
        help.contains("--git-ref"),
        "remote builds must select a revision"
    );
    Ok(())
}

fn git(path: &std::path::Path, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn repository() -> anyhow::Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    git(temp.path(), &["init", "-b", "main"])?;
    git(temp.path(), &[
        "config",
        "user.name",
        "Fixture Committer",
    ])?;
    git(temp.path(), &[
        "config",
        "user.email",
        "committer@example.org",
    ])?;
    std::fs::write(
        temp.path().join("ontology.ttl"),
        "<urn:test:Class> a <http://www.w3.org/2002/07/owl#Class> .\n",
    )?;
    git(temp.path(), &["add", "ontology.ttl"])?;
    git(temp.path(), &[
        "commit",
        "--author",
        "Fixture Author <author@example.org>",
        "-m",
        "Initial ontology",
    ])?;
    Ok(temp)
}

#[tokio::test]
async fn bare_cache_fetches_updates_but_dispatched_collectors_remain_pinned()
-> anyhow::Result<()> {
    use grapharch::publication::{
        collect::git::remote_repository,
        dataset::Dataset,
        writer::GraphWriter,
    };
    let origin = repository()?;
    let url = url::Url::from_directory_path(origin.path())
        .unwrap()
        .to_string();
    let cache = tempfile::tempdir()?;
    let first =
        remote_repository(&url, "main", cache.path(), "urn:test:project")
            .await?;
    assert!(
        !first.root.join("ontology.ttl").exists(),
        "bare cache must not create a checkout"
    );
    assert!(gix::open(&first.root)?.is_bare());
    assert_eq!(first.files.len(), 1);
    std::fs::write(
        origin.path().join("ontology.ttl"),
        "<urn:test:Updated> a <http://www.w3.org/2002/07/owl#Class> .\n",
    )?;
    git(origin.path(), &[
        "commit",
        "-am",
        "Updated ontology",
    ])?;
    let second =
        remote_repository(&url, "main", cache.path(), "urn:test:project")
            .await?;
    assert_eq!(
        first.root, second.root,
        "repeat runs must reuse the cache"
    );
    assert_ne!(first.commit, second.commit);
    assert!(
        remote_repository(
            &url,
            "refs/heads/does-not-exist",
            cache.path(),
            "urn:test:project"
        )
        .await
        .is_err(),
        "missing refs must not silently reuse a stale cached ref"
    );
    let historical = remote_repository(
        &url,
        &first.commit,
        cache.path(),
        "urn:test:project",
    )
    .await?;
    assert_eq!(
        historical.commit, first.commit,
        "explicit commit IDs must stay pinned"
    );
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    first.files[0].collect(&writer).await?;
    first.provenance.collect(&writer).await?;
    drop(writer);
    worker.await??;
    let mut original = dataset.select("SELECT ?g WHERE {GRAPH ?g {<urn:test:Class> a <http://www.w3.org/2002/07/owl#Class>}}".into()).await?;
    assert!(
        original.next().await?.is_some(),
        "first job must still read its pinned blob after a subsequent fetch"
    );
    let mut authors = dataset.select("SELECT ?role ?name WHERE {GRAPH ?g {?attribution <http://www.w3.org/ns/prov#hadRole> ?role ; <http://www.w3.org/ns/prov#agent> ?agent . ?agent <http://xmlns.com/foaf/0.1/name> ?name}}".into()).await?;
    let mut names = Vec::new();
    while let Some(row) = authors.next().await? {
        names.push(format!("{} {}", row["role"], row["name"]));
    }
    assert!(
        names
            .iter()
            .any(|s| s.contains("author") && s.contains("Fixture Author"))
    );
    assert!(names.iter().any(|s| {
        s.contains("committer") && s.contains("Fixture Committer")
    }));
    assert_eq!(
        git(origin.path(), &["status", "--porcelain"])?,
        ""
    );
    Ok(())
}

#[tokio::test]
async fn local_clone_scan_observes_head_without_mutating_uncommitted_files()
-> anyhow::Result<()> {
    let origin = repository()?;
    std::fs::write(
        origin.path().join("ontology.ttl"),
        "uncommitted edit",
    )?;
    let scan = grapharch::publication::collect::git::local_repository(
        origin.path(),
        "urn:test:project",
    )
    .await?;
    assert_eq!(scan.root, origin.path().canonicalize()?);
    assert_eq!(
        scan.commit,
        git(origin.path(), &["rev-parse", "HEAD"])?
    );
    assert_eq!(
        std::fs::read_to_string(origin.path().join("ontology.ttl"))?,
        "uncommitted edit"
    );
    Ok(())
}

#[tokio::test]
async fn local_working_tree_and_cached_remote_feed_the_same_family_pipeline()
-> anyhow::Result<()> {
    use grapharch::publication::{
        ontology_family,
        render::{PublishOutcome, Publisher},
    };
    let origin = repository()?;
    let ontology = r#"@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . @prefix sh:<http://www.w3.org/ns/shacl#> . @prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
<urn:family> a gov:Family; rdfs:label "Example family"@en .
<https://example.org/vocab#> a owl:Ontology; gov:inFamily <urn:family>; rdfs:label "Example ontology"@en; sh:declare [sh:prefix "ex";sh:namespace "https://example.org/vocab#"] .
<https://example.org/vocab#Class> a owl:Class;rdfs:label "Example class"@en;rdfs:comment "The source class."@en ."#;
    std::fs::write(origin.path().join("ontology.ttl"), ontology)?;
    git(origin.path(), &["commit", "-am", "Define family"])?;
    let definitions = tempfile::tempdir()?;
    let config = definitions.path().join("project.ttl");
    std::fs::write(
        &config,
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix pub:<https://ekgf.org/ontology/artifact-publication#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:project> a ga:Project;ga:target <urn:target> . <urn:scope> a ga:Scope . <urn:profile> a ga:Profile;pub:publishesFamily <urn:family> .
<urn:target> a ga:Target;dct:title "Example reference";dct:description "A repository-backed reference.";<http://www.w3.org/2000/01/rdf-schema#comment> "A test documentation preview.";ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer "specification";ga:destination "specification";ga:thisVersion <https://example.org/reference/>;ga:latestVersion <https://example.org/reference/>;ga:license <https://creativecommons.org/publicdomain/zero/1.0/>;ga:publicationStatus "Draft";ga:publicationDate "2026-10-25"^^<http://www.w3.org/2001/XMLSchema#date>;ga:publisher <urn:authority> .
<urn:authority> a ga:Authority;<http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en;ga:shortName "Example" ."#,
    )?;
    std::fs::write(origin.path().join("ontology.ttl"),format!("{ontology}\n<https://example.org/vocab#Draft> a owl:NamedIndividual,<https://example.org/vocab#Class>;rdfs:label \"Draft example\"@en;rdfs:comment \"An uncommitted individual.\"@en ."))?;
    let local = ontology_family::collect_repository(
        origin.path(),
        &config,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    assert_eq!(
        local.term_count, 2,
        "local mode must include working-tree edits"
    );
    let cache = tempfile::tempdir()?;
    let url = url::Url::from_directory_path(origin.path())
        .unwrap()
        .to_string();
    let remote = ontology_family::collect_remote(
        &url,
        "main",
        cache.path(),
        &config,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    assert_eq!(
        remote.term_count, 1,
        "remote mode must use the committed tree"
    );
    std::fs::remove_dir_all(origin.path())?;
    std::fs::remove_dir_all(cache.path())?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(local.dataset)
        .render(&local.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    assert!(html.contains("Named individual: Draft example"));
    assert!(html.contains("Example ontology"));
    Ok(())
}
