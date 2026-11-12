//! A repository's publication hints may register several targets. The family
//! showcase publishes every registered target, or an explicit selection, each
//! into its own destination.
#[path = "support/family_repository.rs"]
mod family_repository;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
    std::path::Path,
};

const ONTOLOGY: &str = r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Multi-target family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>;
 rdfs:label "Multi-target ontology"@en;
 sh:declare [sh:prefix "example"; sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Thing> a owl:Class; rdfs:label "Thing"@en;
 rdfs:comment "Published by every family target."@en .
"#;

/// Three targets share the family profile, each with its own renderer and
/// destination, as in a repository that publishes paginated pages, a PDF
/// edition and a single-page review.
fn hints(website: bool) -> String {
    let target = |key: &str, renderer: &str, destination: &str| {
        format!(
            r#"
<urn:example:{key}> a ga:Target; dct:title "Multi-target family";
 dct:description "One family, several publications.";
 rdfs:comment "Documentation preview.";
 ga:scope <urn:example:scope>; ga:profile <urn:example:profile>;
 ga:renderer ga:{renderer}; ga:destination "{destination}";
 ga:thisVersion <https://example.org/{destination}/>; ga:latestVersion <https://example.org/{destination}/>;
 ga:license <https://creativecommons.org/publicdomain/zero/1.0/>; ga:publicationStatus "Draft";
 ga:publicationDate "2026-11-12"^^<http://www.w3.org/2001/XMLSchema#date>; ga:publisher <urn:example:authority> .
"#
        )
    };
    let (registered, site) = if website {
        (
            ", <urn:example:website>",
            r#"
<urn:example:website> a ga:Target; dct:title "Family website";
 ga:scope <urn:example:website-scope>; ga:profile <urn:example:website-profile>;
 ga:renderer ga:WebsiteRenderer; ga:destination "site" .
<urn:example:website-scope> a ga:Scope .
<urn:example:website-profile> a ga:Profile .
"#,
        )
    } else {
        ("", "")
    };
    format!(
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<urn:example:project> a ga:Project;
 ga:target <urn:example:spec>, <urn:example:pdf>, <urn:example:review>{registered} .
<urn:example:scope> a ga:Scope .
<urn:example:profile> a ga:Profile; pub:publishesFamily <urn:example:family> .
{}{}{}{site}
<urn:example:authority> a <https://ekgf.org/ontology/artifact-governance#Authority>;
 rdfs:label "Example authority"@en; ga:shortName "Example" .
"#,
        target("spec", "SpecificationPagesRenderer", "spec/main"),
        target("pdf", "SpecificationRenderer", "documents/main"),
        target("review", "SpecificationRenderer", "review"),
    )
}

fn repository(root: &Path, website: bool) -> Result<String> {
    let source = root.join("source");
    std::fs::create_dir_all(source.join("publication"))?;
    std::fs::write(source.join("ontology.ttl"), ONTOLOGY)?;
    std::fs::write(
        source.join("publication/grapharch.ttl"),
        hints(website),
    )?;
    Ok(family_repository::bare_copy(&source, &root.join("remote"))?.0)
}

fn showcase(
    root: &Path,
    url: &str,
    targets: Option<&[&str]>,
) -> Result<std::path::PathBuf> {
    let path = root.join("showcase.ttl");
    let mut text = format!(
        "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
         .\n<urn:test:showcase> a ga:Showcase ; ga:gitRepository \"{url}\" ; \
         ga:gitRef \"HEAD\""
    );
    if let Some(targets) = targets {
        text.push_str(" ; ga:showcaseTargets (");
        for target in targets {
            text.push_str(&format!(" <{target}>"));
        }
        text.push_str(" )");
    }
    text.push_str(" .\n");
    std::fs::write(&path, text)?;
    Ok(path)
}

async fn destinations(
    dataset: grapharch::publication::dataset::Dataset,
    plans: &[grapharch::publication::plan::ResolvedPlan],
    output: &Path,
) -> Result<Vec<String>> {
    let publisher = Publisher::new(dataset);
    let mut published = Vec::new();
    for plan in plans {
        let PublishOutcome::Published(publication) =
            publisher.render(plan, output).await?
        else {
            panic!("expected a family publication")
        };
        let directory = publication.directory.strip_prefix(output)?;
        assert!(publication.directory.join("index.html").is_file());
        // A paginated reference places terms on per-ontology pages.
        let mut html = String::new();
        for name in &publication.manifest.files {
            if name.ends_with(".html") {
                html.push_str(&std::fs::read_to_string(
                    publication.directory.join(name),
                )?);
            }
        }
        assert!(
            html.contains("Published by every family target."),
            "{} lacks the family content",
            directory.display()
        );
        published.push(directory.to_str().unwrap().replace('\\', "/"));
    }
    Ok(published)
}

#[tokio::test]
async fn family_showcase_publishes_every_registered_target_in_iri_order()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let url = repository(root.path(), false)?;
    let config = showcase(root.path(), &url, None)?;
    let (dataset, plans) =
        showcases::family(&config, &root.path().join("cache")).await?;
    assert_eq!(
        plans.len(),
        3,
        "every registered target is planned"
    );
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    assert_eq!(
        destinations(dataset, &plans, &output).await?,
        ["documents/main", "review", "spec/main"],
        "targets publish in IRI order, each into its own destination"
    );
    assert!(output.join("documents/main/document.pdf").is_file());
    assert!(output.join("review/document.pdf").is_file());
    Ok(())
}

#[tokio::test]
async fn family_showcase_selects_targets_explicitly_and_rejects_unbuildable_ones()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let url = repository(root.path(), true)?;
    // The family build cannot supply website pages, so it must not publish
    // the website target silently empty or skip it.
    let config = showcase(root.path(), &url, None)?;
    let failure = showcases::family(&config, &root.path().join("cache"))
        .await
        .err()
        .expect("the website target cannot be built from the family");
    let message = format!("{failure:#}");
    assert!(
        message.contains("urn:example:website") &&
            message.contains("urn:example:profile"),
        "the failure names the target and the family profile: {message}"
    );
    let config = showcase(
        root.path(),
        &url,
        Some(&["urn:example:spec", "urn:example:pdf"]),
    )?;
    let (dataset, plans) =
        showcases::family(&config, &root.path().join("cache")).await?;
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    assert_eq!(
        destinations(dataset, &plans, &output).await?,
        ["spec/main", "documents/main"],
        "an explicit selection publishes in its own order"
    );
    for (targets, expected) in [
        (
            &["urn:example:unregistered"][..],
            "not registered",
        ),
        (
            &["urn:example:pdf", "urn:example:pdf"][..],
            "twice",
        ),
    ] {
        let config = showcase(root.path(), &url, Some(targets))?;
        let failure = showcases::family(&config, &root.path().join("cache"))
            .await
            .err()
            .expect("invalid selection must fail");
        assert!(
            format!("{failure:#}").contains(expected),
            "{failure:#}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn single_target_discovery_names_every_registered_target() -> Result<()> {
    use grapharch::publication::{
        collect::git::TargetSelection,
        dataset::Dataset,
        ontology_family::{self, FamilySource},
    };
    let root = tempfile::tempdir()?;
    repository(root.path(), false)?;
    // build-family publishes one target; it must not choose among several.
    let failure = ontology_family::collect_discovered_from(
        Dataset::memory()?,
        FamilySource::Directory(&root.path().join("source")),
        None,
        &TargetSelection::Single,
        2,
    )
    .await
    .err()
    .expect("several registered targets need an explicit selection");
    let message = format!("{failure:#}");
    for target in ["urn:example:spec", "urn:example:pdf", "urn:example:review"]
    {
        assert!(message.contains(target), "{message}");
    }
    Ok(())
}
