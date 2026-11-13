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

/// Whether the repository also registers a website target, and whether its
/// project declares the directory holding the website's pages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    Absent,
    Undeclared,
    Declared,
}

/// Three targets share the family profile, each with its own renderer and
/// destination, as in a repository that publishes paginated pages, a PDF
/// edition and a single-page review.
fn hints(site: Site) -> String {
    let target = |key: &str, renderer: &str, destination: &str| {
        format!(
            r#"
<urn:example:{key}> a ga:Target; dct:title "Multi-target family";
 dct:description "One family, several publications.";
 rdfs:comment "Documentation preview.";
 ga:scope <urn:example:scope>; ga:profile <urn:example:profile>;
 ga:renderer ga:{renderer}; ga:documentType ga:SpecificationDocument; ga:slug "{key}"; ga:destination "{destination}";
 ga:thisVersion <https://example.org/{destination}/>; ga:latestVersion <https://example.org/{destination}/>;
 ga:license <https://creativecommons.org/publicdomain/zero/1.0/>; ga:publicationStatus "Draft";
 ga:publicationDate "2026-11-12"^^<http://www.w3.org/2001/XMLSchema#date>; ga:publisher <urn:example:authority> .
"#
        )
    };
    let content = if site == Site::Declared {
        r#"<urn:example:project> ga:contentSource "publication/site" ."#
    } else {
        ""
    };
    let (registered, site) = if site != Site::Absent {
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
{content}
<urn:example:authority> a <https://ekgf.org/ontology/artifact-governance#Authority>;
 rdfs:label "Example authority"@en; ga:shortName "Example"; ga:slug "example" .
"#,
        target("spec", "SpecificationPagesRenderer", "spec/main"),
        target("pdf", "SpecificationRenderer", "pdf"),
        target("review", "SpecificationRenderer", "review"),
    )
}

fn repository(root: &Path, site: Site) -> Result<String> {
    let source = root.join("source");
    std::fs::create_dir_all(source.join("publication/site"))?;
    std::fs::write(source.join("ontology.ttl"), ONTOLOGY)?;
    std::fs::write(
        source.join("publication/grapharch.ttl"),
        hints(site),
    )?;
    std::fs::write(
        source.join("publication/site/index.md"),
        "---\ntitle: Family home\npermalink: /\n---\n# Family home\n\nRead \
         the [guide](guide.md).\n",
    )?;
    std::fs::write(
        source.join("publication/site/guide.md"),
        "---\ntitle: Guide\n---\n# Guide\n\nWebsite page from the repository. \
         Open the [reference](urn:example:spec).\n",
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
    let publisher = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
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
    let url = repository(root.path(), Site::Absent)?;
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
        ["pdf", "review", "spec/main"],
        "targets publish in IRI order, each into its own destination"
    );
    assert!(only_pdf(&output.join("pdf")).is_file());
    assert!(only_pdf(&output.join("review")).is_file());
    Ok(())
}

#[tokio::test]
async fn family_showcase_selects_targets_explicitly_and_rejects_unbuildable_ones()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let url = repository(root.path(), Site::Undeclared)?;
    // Without a declared page source the website target has no content, so
    // it must not be published empty or skipped silently.
    let config = showcase(root.path(), &url, None)?;
    let failure = showcases::family(&config, &root.path().join("cache"))
        .await
        .err()
        .expect("the website target cannot be built from the family");
    let message = format!("{failure:#}");
    assert!(
        message.contains("urn:example:website") &&
            message.contains("ga:contentSource"),
        "the failure names the target and the missing page source: {message}"
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
        ["spec/main", "pdf"],
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
    repository(root.path(), Site::Absent)?;
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

#[tokio::test]
async fn family_showcase_builds_a_website_from_the_declared_page_source()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let url = repository(root.path(), Site::Declared)?;
    let config = showcase(root.path(), &url, None)?;
    let (dataset, plans) =
        showcases::family(&config, &root.path().join("cache")).await?;
    assert_eq!(
        plans.len(),
        4,
        "the website target is planned too"
    );
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let (family, website) = plans.split_at(3);
    assert_eq!(
        destinations(dataset.clone(), family, &output).await?,
        ["pdf", "review", "spec/main"]
    );
    let PublishOutcome::Published(site) = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&website[0], &output)
        .await?
    else {
        panic!("expected the website publication")
    };
    assert_eq!(site.directory, output.join("site"));
    let home = std::fs::read_to_string(site.directory.join("index.html"))?;
    assert!(home.contains("Family home"), "{home}");
    let pages = site
        .manifest
        .files
        .iter()
        .filter(|file| file.ends_with(".html"))
        .map(|file| std::fs::read_to_string(site.directory.join(file)))
        .collect::<std::io::Result<String>>()?;
    assert!(pages.contains("Website page from the repository."));
    // A target IRI resolves from the page to the target's entry file.
    let guide = site
        .manifest
        .files
        .iter()
        .find(|file| file.ends_with("index.html") && file.contains('/'))
        .expect("the guide page");
    let depth = guide.matches('/').count();
    let guide = std::fs::read_to_string(site.directory.join(guide))?;
    assert!(
        guide.contains(&format!(
            "href=\"{}spec/main/index.html\"",
            "../".repeat(depth + 1)
        )),
        "{guide}"
    );
    Ok(())
}

#[tokio::test]
async fn a_pdf_edition_with_its_own_family_profile_is_bound_too() -> Result<()>
{
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    std::fs::create_dir_all(source.join("publication"))?;
    std::fs::write(source.join("ontology.ttl"), ONTOLOGY)?;
    let review = "ga:profile <urn:example:profile>;\n ga:renderer \
                  ga:SpecificationRenderer; ga:documentType \
                  ga:SpecificationDocument; ga:slug \"review\"; \
                  ga:destination \"review\";";
    let hints = hints(Site::Absent);
    assert_eq!(hints.matches(review).count(), 1, "{hints}");
    let hints = hints.replace(
        review,
        "ga:profile <urn:example:pdf-profile>;\n ga:renderer \
         ga:TypstRenderer; ga:documentType ga:SpecificationDocument; ga:slug \
         \"review\"; ga:destination \"review\";",
    );
    assert!(hints.contains("ga:TypstRenderer"), "{hints}");
    std::fs::write(
        source.join("publication/grapharch.ttl"),
        format!(
            "{hints}\n<urn:example:pdf-profile> a \
             <https://ekgf.org/ontology/grapharch#Profile>; \
             <https://ekgf.org/ontology/artifact-publication#publishesFamily> \
             <urn:example:family>; \
             <https://ekgf.org/ontology/grapharch#theme> \
             <https://ekgf.org/ontology/grapharch#SpecificationTheme> .\n"
        ),
    )?;
    let url =
        family_repository::bare_copy(&source, &root.path().join("remote"))?.0;
    let config = showcase(root.path(), &url, None)?;
    let (dataset, plans) =
        showcases::family(&config, &root.path().join("cache")).await?;
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let publisher = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    for plan in &plans {
        let PublishOutcome::Published(_) =
            publisher.render(plan, &output).await?
        else {
            panic!("expected a publication")
        };
    }
    assert!(only_pdf(&output.join("review")).is_file());
    assert!(
        !output.join("review/index.html").exists(),
        "a PDF-only edition"
    );
    Ok(())
}

/// The one PDF edition published in `directory`.
fn only_pdf(directory: &std::path::Path) -> std::path::PathBuf {
    let mut pdfs = std::fs::read_dir(directory)
        .expect("PDF directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pdf"))
        .collect::<Vec<_>>();
    assert_eq!(
        pdfs.len(),
        1,
        "one PDF edition in {}",
        directory.display()
    );
    pdfs.remove(0)
}
