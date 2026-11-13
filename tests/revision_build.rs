#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

use {
    anyhow::{Result, ensure},
    revision_fixture::{GENERATOR, RevisionFixture},
    std::path::Path,
};

const SPECIFICATION: &str = "urn:dprod-showcase:specification";
const BASE: &str = revision_fixture::PUBLICATION_URL;
const PDF: &str = "urn:dprod-showcase:pdf";

fn fixture() -> Result<RevisionFixture> {
    RevisionFixture::new(Path::new("examples/dprod"))
}

fn succeeded(output: &std::process::Output) -> Result<serde_json::Value> {
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn failed(output: &std::process::Output) -> String {
    assert!(
        !output.status.success(),
        "the build was expected to fail"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn spec(root: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(
        root.join("specification/index.html"),
    )?)
}

#[tokio::test]
async fn exact_revision_build_records_both_pins_and_renders_the_restored_kg()
-> Result<()> {
    let fixture = fixture()?;
    let content = fixture.content();
    for entry in walk(&content)? {
        if entry.starts_with(content.join(".git")) {
            continue;
        }
        assert!(
            !std::fs::read_to_string(&entry)
                .unwrap_or_default()
                .contains("https://ekgf.org/ontology/grapharch#"),
            "the content tree must carry no GraphArch definitions: {}",
            entry.display()
        );
    }
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let manifest = succeeded(&fixture.build(&root, &["--all-targets"]).await?)?;
    assert_eq!(manifest["version"], 2);
    assert_eq!(manifest["project"], revision_fixture::PROJECT);
    assert_eq!(manifest["selection"], "all");
    assert_eq!(manifest["content"]["commit"], fixture.commit);
    assert_eq!(
        manifest["definitions"]["commit"],
        fixture.definitions_commit
    );
    assert_eq!(
        manifest["definitions"]["path"],
        revision_fixture::DEFINITIONS
    );
    let committed = std::fs::read(fixture.definitions())?;
    assert_eq!(
        manifest["definitions"]["sha256"],
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&committed))
    );
    assert_eq!(
        manifest["definitions"]["blob"],
        family_repository::git(&fixture.controlling(), &[
            "rev-parse",
            &format!("HEAD:{}", revision_fixture::DEFINITIONS),
        ])?
    );
    assert_eq!(manifest["generator"]["revision"], GENERATOR);
    assert_eq!(manifest["graph"], "documentation.nq");
    let publications = manifest["publications"].as_array().unwrap();
    assert_eq!(
        publications
            .iter()
            .map(|publication| publication["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [PDF, SPECIFICATION, "urn:dprod-showcase:website"],
        "--all-targets selects registered targets in IRI order"
    );
    assert!(root.join("documentation.nq").is_file());
    assert!(root.join("build-provenance.json").is_file());
    assert!(!root.join("gallery.html").exists());
    let html = spec(&root)?;
    for expected in [
        "DataProduct",
        "OWL classes",
        "SHACL property shapes",
        "Dataset schema example",
        "2026-06-01",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    for empty in ["Annotation properties", "Named individuals"] {
        assert!(
            !html.contains(empty),
            "an empty reference component must produce no section: {empty}"
        );
    }
    Ok(())
}

/// Regression: definitions were read from the content commit.
#[tokio::test]
async fn definitions_are_read_from_the_controlling_revision_not_the_content_commit()
-> Result<()> {
    let mut fixture = fixture()?;
    let trap = std::fs::read_to_string(fixture.definitions())?.replace(
        "DPROD ontology and SHACL reference\"",
        "TRAP DEFINITIONS FROM THE CONTENT COMMIT\"",
    );
    fixture.edit_content(revision_fixture::DEFINITIONS, |_| Ok(trap))?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    succeeded(&fixture.build(&root, &["--target", SPECIFICATION]).await?)?;
    let html = spec(&root)?;
    assert!(
        !html.contains("TRAP DEFINITIONS FROM THE CONTENT COMMIT"),
        "definitions were read from the content commit"
    );
    assert!(html.contains("DPROD ontology and SHACL reference"));
    Ok(())
}

#[tokio::test]
async fn failed_builds_do_not_promote_a_partial_publication() -> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_definitions(|text| {
        Ok(text.replace(
            "ga:sourcePath \"status.md\"",
            "ga:sourcePath \"absent.md\"",
        ))
    })?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let error = failed(&fixture.build(&root, &["--all-targets"]).await?);
    assert!(
        error.contains("missing required Git blob absent.md"),
        "{error}"
    );
    assert!(
        !root.exists(),
        "failed collection must not promote output"
    );
    assert_eq!(
        std::fs::read_dir(output.path())?.count(),
        0,
        "the staging directory must be removed"
    );
    Ok(())
}

#[tokio::test]
async fn unverified_definitions_are_rejected_before_any_transport() -> Result<()>
{
    let mut fixture = fixture()?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let committed = std::fs::read_to_string(fixture.definitions())?;
    std::fs::write(
        fixture.definitions(),
        format!("{committed}\n# edit\n"),
    )?;
    let error = failed(&fixture.build(&root, &["--all-targets"]).await?);
    assert!(
        error
            .contains("definitions differ from committed controlling revision"),
        "{error}"
    );
    std::fs::write(fixture.definitions(), &committed)?;
    let untracked = fixture.controlling().join("publication/untracked.ttl");
    std::fs::write(&untracked, &committed)?;
    let error = failed(
        &fixture
            .build_with(&root, &["--all-targets"], &[(
                "--definitions",
                untracked.to_str().unwrap(),
            )])
            .await?,
    );
    assert!(error.contains("not committed"), "{error}");
    std::fs::remove_file(untracked)?;
    let previous = fixture.definitions_commit.clone();
    fixture.edit_definitions(|text| Ok(format!("{text}\n# next\n")))?;
    let error = failed(
        &fixture
            .build_with(&root, &["--all-targets"], &[(
                "--definitions-commit",
                &previous,
            )])
            .await?,
    );
    assert!(
        error.contains("differs from definitions commit"),
        "{error}"
    );
    assert!(
        !fixture.cache().exists(),
        "definitions are verified before any Git transport"
    );
    assert!(!root.exists());
    Ok(())
}

#[tokio::test]
async fn mutable_references_are_rejected_before_network_and_cache_creation()
-> Result<()> {
    let fixture = fixture()?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    for (argument, value) in [
        ("--git-commit", "main"),
        ("--git-commit", "develop"),
        ("--git-commit", &fixture.commit[..12]),
        ("--git-commit", &fixture.commit.to_uppercase()),
        ("--definitions-commit", "HEAD"),
        ("--generator-revision", "main"),
    ] {
        let error = failed(
            &fixture
                .build_with(&root, &["--all-targets"], &[
                    (argument, value),
                    (
                        "--git-repository",
                        "https://example.invalid/never.git",
                    ),
                ])
                .await?,
        );
        assert!(
            error.contains(&format!(
                "{argument} requires an exact lowercase 40-character Git \
                 commit"
            )),
            "{error}"
        );
    }
    assert!(!fixture.cache().exists());
    assert!(!root.exists());
    Ok(())
}

#[tokio::test]
async fn explicit_publication_date_replaces_derivation_for_an_issued_less_ontology()
-> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_content("ontology.ttl", |text| {
        let kept = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("dct:issued \""))
            .collect::<Vec<_>>()
            .join("\n");
        ensure!(
            kept.len() < text.len(),
            "fixture ontology declares dct:issued"
        );
        Ok(kept)
    })?;
    let ontology =
        std::fs::read_to_string(fixture.content().join("ontology.ttl"))?;
    let output = tempfile::tempdir()?;
    let error = failed(
        &fixture
            .build(&output.path().join("derived"), &["--all-targets"])
            .await?,
    );
    assert!(
        error.contains("is missing <http://purl.org/dc/terms/issued>"),
        "{error}"
    );
    fixture.edit_definitions(|text| {
        Ok(text.replace(
            "ga:metadataSource ex:collector:ontology ; ga:derivedMetadata \
             ga:publicationDate .",
            "ga:publicationDate \"2025-01-02\"^^<http://www.w3.org/2001/XMLSchema#date> .",
        ))
    })?;
    let root = output.path().join("explicit");
    succeeded(&fixture.build(&root, &["--all-targets"]).await?)?;
    assert!(spec(&root)?.contains("2025-01-02"));
    assert_eq!(
        std::fs::read_to_string(fixture.content().join("ontology.ttl"))?,
        ontology,
        "native RDF is never rewritten"
    );
    Ok(())
}

#[tokio::test]
async fn derived_metadata_conflicting_with_an_explicit_value_is_an_error()
-> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_definitions(|text| {
        Ok(text.replace(
            "ga:derivedMetadata ga:publicationDate .",
            "ga:derivedMetadata ga:publicationDate, ga:license .",
        ))
    })?;
    let output = tempfile::tempdir()?;
    let error = failed(
        &fixture
            .build(&output.path().join("published"), &[
                "--all-targets",
            ])
            .await?,
    );
    assert!(
        error.contains("conflicts with an explicit value"),
        "{error}"
    );
    Ok(())
}

#[tokio::test]
async fn specification_only_definitions_need_no_website_target() -> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_definitions(|text| {
        Ok(text.replace(
            "ga:target ex:specification, ex:website, ex:pdf ;",
            "ga:target ex:specification, ex:pdf ;",
        ))
    })?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let manifest = succeeded(&fixture.build(&root, &["--all-targets"]).await?)?;
    assert_eq!(
        manifest["publications"].as_array().unwrap().len(),
        2
    );
    assert!(!root.join("website").exists());
    Ok(())
}

#[tokio::test]
async fn selected_targets_publish_in_order_and_unregistered_targets_fail()
-> Result<()> {
    let fixture = fixture()?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let manifest = succeeded(
        &fixture
            .build(&root, &[
                "--target",
                SPECIFICATION,
                "--target",
                PDF,
            ])
            .await?,
    )?;
    assert_eq!(
        manifest["selection"],
        serde_json::json!([SPECIFICATION, PDF])
    );
    assert_eq!(
        manifest["publications"]
            .as_array()
            .unwrap()
            .iter()
            .map(|publication| publication["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [SPECIFICATION, PDF]
    );
    assert!(!root.join("website").exists());
    let error = failed(
        &fixture
            .build(&output.path().join("unregistered"), &[
                "--target",
                "urn:dprod-showcase:release",
            ])
            .await?,
    );
    assert!(error.contains("is not registered by"), "{error}");
    let error = failed(
        &fixture
            .build(&output.path().join("unselected"), &[])
            .await?,
    );
    assert!(
        error.contains("required arguments were not provided") &&
            error.contains("--target <IRI>"),
        "exactly one of --target or --all-targets is required: {error}"
    );
    Ok(())
}

#[tokio::test]
async fn content_cannot_define_or_redefine_publications() -> Result<()> {
    for (assertion, expected) in [
        (
            "<urn:dprod-showcase:project> \
             <https://ekgf.org/ontology/grapharch#target> \
             <urn:dprod-showcase:injected> .",
            "publication definition predicate",
        ),
        (
            "<urn:dprod-showcase:injected> a \
             <https://ekgf.org/ontology/grapharch#Placement> .",
            "publication definition class",
        ),
        (
            "<urn:dprod-showcase:specification> \
             <http://purl.org/dc/terms/title> \"INJECTED TITLE\" .",
            "assertion about a defined publication resource",
        ),
    ] {
        let mut fixture = fixture()?;
        fixture.edit_content("ontology.ttl", |text| {
            Ok(format!("{text}\n{assertion}\n"))
        })?;
        let output = tempfile::tempdir()?;
        let error = failed(
            &fixture
                .build(&output.path().join("published"), &["--all-targets"])
                .await?,
        );
        assert!(
            error.contains(expected) &&
                error.contains("only the controlling definitions may define \
                                publications"),
            "{error}"
        );
    }
    Ok(())
}

/// Deployment location is a build input, never source content: each selected
/// target's `ga:thisVersion` derives from `--publication-url` and its
/// destination, with the renderer's entry point.
#[tokio::test]
async fn publication_url_derives_the_version_of_every_selected_target()
-> Result<()> {
    let fixture = fixture()?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let manifest = succeeded(&fixture.build(&root, &["--all-targets"]).await?)?;
    assert_eq!(manifest["publication_url"], BASE);
    let html = spec(&root)?;
    assert!(
        html.contains(&format!(
            "<dt>This version:</dt><dd><a href=\"{BASE}specification/\""
        )),
        "the specification is published at its destination directory"
    );
    let website = std::fs::read_to_string(root.join("website/index.html"))?;
    assert!(
        website.contains(&format!(
            "<link rel=\"canonical\" href=\"{BASE}website/\">"
        )),
        "the website canonical location derives from the same base"
    );
    let graph = std::fs::read_to_string(root.join("documentation.nq"))?;
    for (target, version) in [
        (SPECIFICATION, format!("{BASE}specification/")),
        (
            "urn:dprod-showcase:website",
            format!("{BASE}website/"),
        ),
        (PDF, format!("{BASE}pdf/ekgf-spec-dprod.pdf")),
    ] {
        assert!(
            graph.contains(&format!(
                "<{target}> <https://ekgf.org/ontology/grapharch#thisVersion> \
                 <{version}>"
            )),
            "{target} must record its derived version {version}"
        );
    }
    assert!(
        !html.contains("showcases/artifacts/dprod"),
        "no deployment location may come from the definitions"
    );
    Ok(())
}

#[tokio::test]
async fn definitions_cannot_declare_the_version_of_a_selected_target()
-> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_definitions(|text| {
        Ok(format!(
            "{text}\n<{PDF}> <https://ekgf.org/ontology/grapharch#thisVersion> \
             <https://example.org/elsewhere/document.pdf> .\n"
        ))
    })?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("pdf");
    let error = failed(&fixture.build(&root, &["--target", PDF]).await?);
    assert!(
        error.contains(&format!(
            "<{PDF}> declares ga:thisVersion <https://example.org/elsewhere/document.pdf>"
        )) && error.contains("--publication-url"),
        "an explicit version conflicts with the derived one: {error}"
    );
    assert!(!root.exists());
    succeeded(
        &fixture
            .build(&output.path().join("specification"), &[
                "--target",
                SPECIFICATION,
            ])
            .await?,
    )?;
    Ok(())
}

#[tokio::test]
async fn publication_urls_must_be_absolute_directories_before_any_transport()
-> Result<()> {
    let fixture = fixture()?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    for value in [
        "https://example.org/dprod/spec/develop",
        "ftp://example.org/dprod/",
        "https://reader@example.org/dprod/",
        "https://example.org/dprod/?preview=1",
        "https://example.org/dprod/#top",
        "dprod/spec/develop/",
    ] {
        let error = failed(
            &fixture
                .build_with(&root, &["--all-targets"], &[
                    ("--publication-url", value),
                    (
                        "--git-repository",
                        "https://example.invalid/never.git",
                    ),
                ])
                .await?,
        );
        assert!(
            error.contains("--publication-url") &&
                error.contains(
                    "an absolute HTTP(S) directory URL ending in /, without \
                     credentials, query or fragment"
                ),
            "{value}: {error}"
        );
    }
    assert!(!fixture.cache().exists());
    assert!(!root.exists());
    Ok(())
}

/// Authored links between co-built artifacts stay relative to the page while
/// the canonical version beside them is the derived absolute location.
#[tokio::test]
async fn authored_edition_links_stay_relative_beside_derived_versions()
-> Result<()> {
    let mut fixture = fixture()?;
    fixture.edit_definitions(|text| {
        Ok(format!(
            r##"{text}
ex:specification ga:metadataRow ex:row:version, ex:row:latest, ex:row:editions .
ex:row:version a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:row:latest a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:row:editions a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ; dct:title "PDF edition:" ;
    ga:body """<a href="../pdf/ekgf-spec-dprod.pdf">Review PDF</a>""" .
"##
        ))
    })?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    succeeded(&fixture.build(&root, &["--all-targets"]).await?)?;
    let page = root.join("specification/index.html");
    let html = std::fs::read_to_string(&page)?;
    let document = scraper::Html::parse_document(&html);
    let selector =
        scraper::Selector::parse("dl.publication-metadata a[href]").unwrap();
    let hrefs: Vec<_> = document
        .select(&selector)
        .map(|link| link.attr("href").unwrap().to_owned())
        .collect();
    assert!(
        hrefs.contains(&format!("{BASE}specification/")),
        "{hrefs:?}"
    );
    assert!(
        hrefs
            .iter()
            .any(|href| href == "../pdf/ekgf-spec-dprod.pdf"),
        "the authored edition link must stay relative: {hrefs:?}"
    );
    let resolved = url::Url::from_file_path(&page)
        .unwrap()
        .join("../pdf/ekgf-spec-dprod.pdf")?
        .to_file_path()
        .unwrap();
    assert!(
        resolved.starts_with(&root) &&
            std::fs::read(&resolved)?.starts_with(b"%PDF-"),
        "the relative edition link resolves to the built PDF: {}",
        resolved.display()
    );
    Ok(())
}

fn walk(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}
