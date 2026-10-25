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
