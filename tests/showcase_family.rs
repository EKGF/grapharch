use anyhow::Result;

#[test]
fn published_family_showcase_follows_the_remote_default_branch_instead_of_corpus()
-> Result<()> {
    let config: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string("examples/ekg-ontologies/showcase.yaml")?,
    )?;
    assert_eq!(config["source"]["kind"].as_str(), Some("git"));
    assert_eq!(
        config["source"]["url"].as_str(),
        Some("https://github.com/EKGF/ekg-ontologies.git")
    );
    assert_eq!(config["source"]["ref"].as_str(), Some("HEAD"));
    assert!(
        include_str!("../examples/showcases.ttl")
            .contains("ga:source \"ekg-ontologies/showcase.yaml\"")
    );
    Ok(())
}

#[path = "support/family_repository.rs"]
mod family_repository;

fn configuration(
    root: &std::path::Path,
    url: &str,
    reference: &str,
) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(root)?;
    let path = root.join("showcase.yaml");
    std::fs::write(
        &path,
        serde_yaml::to_string(&serde_json::json!({
            "definitions": "project.ttl", "project": "urn:example:project", "target": "urn:example:target",
            "source": {"kind": "git", "url": url, "ref": reference}
        }))?,
    )?;
    std::fs::write(
        root.join("project.ttl"),
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:example:project> a ga:Project; ga:target <urn:example:target> .
<urn:example:scope> a ga:Scope .
<urn:example:profile> a ga:Profile; pub:publishesFamily <urn:example:family> .
<urn:example:target> a ga:Target; dct:title "Remote family"; dct:description "A repository-backed example.";
 <http://www.w3.org/2000/01/rdf-schema#comment> "Documentation preview.";
 ga:scope <urn:example:scope>; ga:profile <urn:example:profile>; ga:renderer "specification"; ga:destination "specification";
 ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>;
 ga:license <https://creativecommons.org/publicdomain/zero/1.0/>; ga:publicationStatus "Draft";
 ga:publicationDate "2026-10-25"^^<http://www.w3.org/2001/XMLSchema#date>; ga:publisher <urn:example:authority> .
<urn:example:authority> a ga:Authority; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en; ga:shortName "Example" .
"#,
    )?;
    Ok(path)
}

async fn contains_body(
    dataset: &grapharch::publication::dataset::Dataset,
    text: &str,
) -> Result<bool> {
    let mut rows = dataset
        .select(format!(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body \
             WHERE {{ GRAPH ?g {{?fragment ga:body ?body}} \
             FILTER(CONTAINS(STR(?body),{text:?})) }} LIMIT 1"
        ))
        .await?;
    Ok(rows.next().await?.is_some())
}

async fn contains_commit(
    dataset: &grapharch::publication::dataset::Dataset,
    commit: &str,
) -> Result<bool> {
    let mut rows=dataset.select(format!("SELECT ?revision WHERE {{ GRAPH ?g {{ ?revision <http://purl.org/dc/terms/identifier> {commit:?}; <http://www.w3.org/ns/prov#wasDerivedFrom> ?repository }} }} LIMIT 1")).await?;
    Ok(rows.next().await?.is_some())
}

#[tokio::test]
async fn repository_showcase_refreshes_head_reuses_cache_and_keeps_pinned_revisions()
-> Result<()> {
    use grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    };
    let root = tempfile::tempdir()?;
    let initial = root.path().join("initial");
    std::fs::create_dir(&initial)?;
    let ontology = |definition: &str| {
        format!(
            r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Example family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>; rdfs:label "Example ontology"@en; sh:declare [sh:prefix "example"; sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Thing> a owl:Class; rdfs:label "Thing"@en; rdfs:comment "{definition}"@en .
"#
        )
    };
    std::fs::write(
        initial.join("ontology.ttl"),
        ontology("First remote definition"),
    )?;
    let repository = root.path().join("repository");
    let (url, first_commit) =
        family_repository::bare_copy(&initial, &repository)?;
    let config_root = root.path().join("configuration");
    let config = configuration(&config_root, &url, "HEAD")?;
    // A conflicting copied corpus must never be selected instead of configured
    // Git.
    std::fs::create_dir(config_root.join("sources"))?;
    std::fs::write(
        config_root.join("sources/ontology.ttl"),
        ontology("Copied corpus must not appear"),
    )?;
    let cache = root.path().join("cache");
    let (first, _) = showcases::family(&config, &cache).await?;
    assert!(contains_body(&first, "First remote definition").await?);
    assert!(!contains_body(&first, "Copied corpus must not appear").await?);
    assert!(contains_commit(&first, &first_commit).await?);
    let repositories =
        std::fs::read_dir(&cache)?.collect::<std::io::Result<Vec<_>>>()?;
    let directories: Vec<_> = repositories
        .iter()
        .filter(|entry| entry.path().is_dir())
        .collect();
    assert_eq!(directories.len(), 1);
    let cached = directories[0].path();
    assert!(gix::open(&cached)?.is_bare());
    std::fs::write(
        cached.join("cache-reuse-proof"),
        "retain this cache",
    )?;
    let checkout = repository.join("checkout");
    std::fs::write(
        checkout.join("ontology.ttl"),
        ontology("Latest remote definition"),
    )?;
    family_repository::git(&checkout, &[
        "commit",
        "-am",
        "Update remote definition",
    ])?;
    family_repository::git(&checkout, &[
        "push",
        repository.join("remote.git").to_str().unwrap(),
        "HEAD:main",
    ])?;
    let latest_commit =
        family_repository::git(&checkout, &["rev-parse", "HEAD"])?;
    let (latest, plans) = showcases::family(&config, &cache).await?;
    assert!(contains_body(&latest, "Latest remote definition").await?);
    assert!(!contains_body(&latest, "First remote definition").await?);
    assert!(contains_commit(&latest, &latest_commit).await?);
    assert!(cached.join("cache-reuse-proof").is_file());
    configuration(&config_root, &url, &first_commit)?;
    let (pinned, _) = showcases::family(&config, &cache).await?;
    assert!(contains_body(&pinned, "First remote definition").await?);
    assert!(!contains_body(&pinned, "Latest remote definition").await?);
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    latest.export(archive.clone()).await?;
    std::fs::remove_dir_all(repository.join("remote.git"))?;
    let _failure = showcases::family(&config, &cache).await.err().expect(
        "unavailable repository must fail even with a cache and corpus",
    );
    std::fs::remove_dir_all(root.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("expected restored publication")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    assert!(html.contains("Latest remote definition"));
    assert!(publication.directory.join("document.pdf").is_file());
    Ok(())
}

#[tokio::test]
async fn repository_publication_hints_can_live_inside_an_ontology_without_local_definitions()
-> Result<()> {
    use grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    };
    let root = tempfile::tempdir()?;
    let bootstrap = root.path().join("bootstrap");
    configuration(&bootstrap, "file:///unused", "HEAD")?;
    let hints = std::fs::read_to_string(bootstrap.join("project.ttl"))?
        .replace(
            "ga:destination \"specification\"",
            "ga:destination \"upstream/spec\"",
        );
    let source = root.path().join("ontology");
    std::fs::create_dir(&source)?;
    std::fs::write(
        source.join("vocabulary.ttl"),
        format!(
            r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> . @prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Embedded hints family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>; rdfs:label "Embedded hints ontology"@en; sh:declare [sh:prefix "example"; sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Thing> a owl:Class; rdfs:label "Thing"@en; rdfs:comment "Defined alongside its publication hints."@en .
{hints}
"#
        ),
    )?;
    let (url, commit) =
        family_repository::bare_copy(&source, &root.path().join("remote"))?;
    let config = root.path().join("showcase.yaml");
    std::fs::write(
        &config,
        serde_yaml::to_string(
            &serde_json::json!({"source":{"kind":"git","url":url,"ref":"HEAD"}}),
        )?,
    )?;
    std::fs::remove_dir_all(bootstrap)?;
    let (dataset, plans) =
        showcases::family(&config, &root.path().join("cache")).await?;
    assert!(contains_commit(&dataset, &commit).await?);
    assert!(
        contains_body(
            &dataset,
            "Defined alongside its publication hints."
        )
        .await?
    );
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("expected discovered publication")
    };
    assert_eq!(
        publication.directory,
        output.path().canonicalize()?.join("upstream/spec")
    );
    // Preparing resolves HEAD once. Advancing the upstream must not change that
    // scan.
    let prepared = grapharch::publication::collect::git::remote_publication(
        &url,
        "HEAD",
        &root.path().join("cache"),
        None,
    )
    .await?;
    let checkout = root.path().join("remote/checkout");
    let original = std::fs::read_to_string(checkout.join("vocabulary.ttl"))?;
    std::fs::write(
        checkout.join("vocabulary.ttl"),
        original.replace(
            "Defined alongside its publication hints.",
            "The upstream moved after discovery.",
        ),
    )?;
    family_repository::git(&checkout, &[
        "commit",
        "-am",
        "Move upstream after discovery",
    ])?;
    family_repository::git(&checkout, &[
        "push",
        root.path().join("remote/remote.git").to_str().unwrap(),
        "HEAD:main",
    ])?;
    let built = grapharch::publication::ontology_family::collect_prepared(
        grapharch::publication::dataset::Dataset::memory()?,
        prepared,
        2,
    )
    .await?;
    assert!(contains_commit(&built.dataset, &commit).await?);
    assert!(
        contains_body(
            &built.dataset,
            "Defined alongside its publication hints."
        )
        .await?
    );
    assert!(
        !contains_body(
            &built.dataset,
            "The upstream moved after discovery."
        )
        .await?
    );
    // Ambiguous hints require explicit selection; an explicit RDF path may
    // select one project.
    std::fs::write(checkout.join("another-project.ttl"),"<urn:another:project> a <https://ekgf.org/ontology/grapharch#Project>; <http://purl.org/dc/terms/title> \"Another project\"; <https://ekgf.org/ontology/grapharch#target> <urn:example:target> .")?;
    family_repository::git(&checkout, &["add", "another-project.ttl"])?;
    family_repository::git(&checkout, &[
        "commit",
        "-m",
        "Add another publication project",
    ])?;
    family_repository::git(&checkout, &[
        "push",
        root.path().join("remote/remote.git").to_str().unwrap(),
        "HEAD:main",
    ])?;
    let failure = showcases::family(&config, &root.path().join("cache"))
        .await
        .err()
        .expect("ambiguous projects require explicit selection");
    assert!(format!("{failure:#}").contains("exactly one ga:Project"));
    let mut selection: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&config)?)?;
    selection["source"]["config"] = "vocabulary.ttl".into();
    std::fs::write(&config, serde_yaml::to_string(&selection)?)?;
    let (selected, _) =
        showcases::family(&config, &root.path().join("cache")).await?;
    assert!(
        contains_body(&selected, "The upstream moved after discovery.").await?
    );
    Ok(())
}

#[tokio::test]
async fn filesystem_discovery_preserves_the_selected_dataset_for_directory_and_checkout()
-> Result<()> {
    use grapharch::publication::{
        dataset::Dataset,
        ontology_family::{self, FamilySource},
    };
    let root = tempfile::tempdir()?;
    let bootstrap = root.path().join("bootstrap");
    configuration(&bootstrap, "file:///unused", "HEAD")?;
    let source = root.path().join("source");
    std::fs::create_dir_all(source.join("publication"))?;
    std::fs::copy(
        bootstrap.join("project.ttl"),
        source.join("publication/grapharch.ttl"),
    )?;
    std::fs::write(
        source.join("vocabulary.ttl"),
        r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> . @prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Filesystem family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>; rdfs:label "Filesystem ontology"@en; sh:declare [sh:prefix "example";sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Thing> a owl:Class;rdfs:label "Thing"@en;rdfs:comment "Filesystem term definition"@en .
"#,
    )?;
    let dataset = Dataset::memory()?;
    let observed = dataset.clone();
    ontology_family::collect_discovered_from(
        dataset,
        FamilySource::Directory(&source),
        None,
        2,
    )
    .await?;
    assert!(contains_body(&observed, "Filesystem term definition").await?);
    let remote = root.path().join("repository");
    family_repository::bare_copy(&source, &remote)?;
    let dataset = Dataset::memory()?;
    let observed = dataset.clone();
    ontology_family::collect_discovered_from(
        dataset,
        FamilySource::LocalRepository(&remote.join("checkout")),
        Some(std::path::Path::new("publication/grapharch.ttl")),
        2,
    )
    .await?;
    assert!(contains_body(&observed, "Filesystem term definition").await?);
    Ok(())
}
