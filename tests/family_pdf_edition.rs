//! A family publishes its PDF edition from an explicit Typst target, without
//! a single-page HTML copy.
use {
    anyhow::Result,
    grapharch::publication::{
        ontology_family,
        pdf_name::Edition,
        render::{Publication, PublishOutcome, Publisher},
    },
    std::path::PathBuf,
};

const ONTOLOGY: &str = r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:example:family> a gov:Family; rdfs:label "Edition family"@en .
<https://example.org/ontology#> a owl:Ontology; gov:inFamily <urn:example:family>;
 rdfs:label "Edition ontology"@en;
 sh:declare [sh:prefix "example"; sh:namespace "https://example.org/ontology#"] .
<https://example.org/ontology#Thing> a owl:Class; rdfs:label "Thing"@en;
 rdfs:comment "Published in the pages and in the PDF edition."@en .
"#;

const PAGES: &str = "urn:example:spec";
const PDF: &str = "urn:example:pdf";

/// One project registering a paginated specification and a Typst PDF
/// edition. `pdf_extra` adds statements to the PDF target, such as its
/// composition with the pages target.
fn project_definitions(pdf_destination: &str, pdf_extra: &str) -> String {
    let target = |key: &str,
                  profile: &str,
                  renderer: &str,
                  destination: &str,
                  extra: &str| {
        format!(
            r#"
<urn:example:{key}> a ga:Target; dct:title "Edition family";
 dct:description "One family, one specification and its PDF edition.";
 rdfs:comment "Documentation preview.";
 ga:scope <urn:example:scope>; ga:profile <urn:example:{profile}>;
 ga:renderer ga:{renderer}; ga:documentType ga:SpecificationDocument; ga:slug "{key}"; ga:destination "{destination}";
 ga:thisVersion <https://example.org/spec/main/>; ga:latestVersion <https://example.org/spec/main/>;
 ga:license <https://creativecommons.org/publicdomain/zero/1.0/>; ga:publicationStatus "Editor's Draft";
 ga:publicationDate "2026-11-12"^^<http://www.w3.org/2001/XMLSchema#date>; ga:publisher <urn:example:authority> {extra}.
"#
        )
    };
    format!(
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
<urn:example:project> a ga:Project; ga:target <{PAGES}>, <{PDF}> .
<urn:example:scope> a ga:Scope .
<urn:example:profile> a ga:Profile, pub:PublicationProfile; pub:publishesFamily <urn:example:family> .
<urn:example:pdf-profile> a ga:Profile, pub:PublicationProfile; pub:publishesFamily <urn:example:family>;
 ga:theme ga:SpecificationTheme .
{}{}
<urn:example:authority> a <https://ekgf.org/ontology/artifact-governance#Authority>;
 rdfs:label "Example authority"@en; ga:shortName "Example"; ga:slug "example" .
"#,
        target(
            "spec",
            "profile",
            "SpecificationPagesRenderer",
            "spec/main",
            ""
        ),
        target(
            "pdf",
            "pdf-profile",
            "TypstRenderer",
            pdf_destination,
            pdf_extra
        ),
    )
}

struct Fixture {
    _root:       tempfile::TempDir,
    source:      PathBuf,
    definitions: PathBuf,
    output:      PathBuf,
}

impl Fixture {
    fn new(pdf_destination: &str, pdf_extra: &str) -> Result<Self> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        std::fs::create_dir(&source)?;
        std::fs::write(source.join("ontology.ttl"), ONTOLOGY)?;
        let definitions = root.path().join("grapharch.ttl");
        std::fs::write(
            &definitions,
            project_definitions(pdf_destination, pdf_extra),
        )?;
        let output = root.path().join("output");
        std::fs::create_dir(&output)?;
        let output = output.canonicalize()?;
        Ok(Self { _root: root, source, definitions, output })
    }

    /// Build and render one target from a fresh KG, as `build-family` does.
    async fn publish(&self, target: &str) -> Result<Publication> {
        self.publish_edition(target, Some(Edition::Default)).await
    }

    async fn publish_edition(
        &self,
        target: &str,
        edition: Option<Edition>,
    ) -> Result<Publication> {
        let built = ontology_family::collect(
            &self.source,
            &self.definitions,
            "urn:example:project",
            target,
            2,
        )
        .await?;
        let PublishOutcome::Published(publication) =
            Publisher::new(built.dataset)
                .with_edition(
                    grapharch::publication::pdf_name::Edition::Default,
                )
                .with_edition_input(edition)
                .render(&built.plan, &self.output)
                .await?
        else {
            panic!("expected a published family target")
        };
        Ok(*publication)
    }
}

#[tokio::test]
async fn family_typst_target_publishes_only_a_specification_styled_pdf()
-> Result<()> {
    let fixture = Fixture::new("pdf", "")?;
    let publication = fixture.publish(PDF).await?;
    assert_eq!(publication.directory, fixture.output.join("pdf"));
    let files = &publication.manifest.files;
    assert_eq!(
        files,
        &["example-spec-pdf.pdf".to_owned()],
        "a PDF target owns exactly its named PDF edition"
    );
    assert!(
        !files.iter().any(|file| file.ends_with(".html")),
        "a Typst family target publishes no HTML copy: {files:?}"
    );
    let pdf = std::fs::read(publication.pdf().expect("published PDF edition"))?;
    assert!(pdf.starts_with(b"%PDF"));
    let typst = std::fs::read_to_string(publication.typst_source())?;
    assert!(
        typst.contains(
            "// Theme: https://ekgf.org/ontology/grapharch#SpecificationTheme"
        ),
        "the profile's ga:theme selects the specification look"
    );
    for text in [
        "Edition ontology",
        "Published in the pages and in the PDF edition.",
        "Editor's Draft",
    ] {
        assert!(
            typst.contains(text),
            "the PDF edition lacks {text}"
        );
    }
    Ok(())
}

/// Competing English labels from two sources are a reported quality warning
/// in every publishing path: the family build and a later render of its
/// exported KG both succeed and both report the finding.
#[tokio::test]
async fn competing_labels_are_one_warning_rule_for_build_and_render()
-> Result<()> {
    use tokio::process::Command;
    let fixture = Fixture::new("pdf", "")?;
    std::fs::write(
        fixture.source.join("alias.ttl"),
        "<https://example.org/ontology#Thing> \
         <http://www.w3.org/2000/01/rdf-schema#label> \"RDF Thing\"@en .\n",
    )?;
    let binary = env!("CARGO_BIN_EXE_grapharch");
    let built = Command::new(binary)
        .args(["publication", "build-family", "--source"])
        .arg(&fixture.source)
        .arg("--definitions")
        .arg(&fixture.definitions)
        .args(["--project", "urn:example:project", "--target", PAGES])
        .arg("--output-root")
        .arg(&fixture.output)
        .arg("--json")
        .output()
        .await?;
    let stderr = String::from_utf8_lossy(&built.stderr).into_owned();
    assert!(built.status.success(), "{stderr}");
    assert!(
        stderr.contains("competing-source-annotations"),
        "build-family reports the warning: {stderr}"
    );
    let report: serde_json::Value = serde_json::from_slice(&built.stdout)?;
    let store = fixture.output.join("store");
    let imported = Command::new(binary)
        .args(["publication", "import", "--store"])
        .arg(&store)
        .arg("--input")
        .arg(fixture.output.join("documentation.nq"))
        .output()
        .await?;
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let rendered = Command::new(binary)
        .args(["publication", "render", "--store"])
        .arg(&store)
        .args(["--plan", report["plan"].as_str().unwrap()])
        .args(["--snapshot", report["snapshot"].as_str().unwrap()])
        .arg("--output-root")
        .arg(fixture.output.join("rerendered"))
        .output()
        .await?;
    let stderr = String::from_utf8_lossy(&rendered.stderr).into_owned();
    assert!(
        rendered.status.success(),
        "a warning-only publication renders: {stderr}"
    );
    assert!(
        stderr.contains("competing-source-annotations"),
        "{stderr}"
    );
    Ok(())
}

/// PDF editions of different builds never overwrite each other: only the
/// declared default-branch build is unsuffixed, every other build names its
/// edition, and a PDF build without an edition input fails.
#[tokio::test]
async fn editions_of_one_target_get_distinct_paths_and_require_an_input()
-> Result<()> {
    let fixture = Fixture::new("pdf", "")?;
    let mut paths = Vec::new();
    for edition in ["default", "pr-23", "feature-x", "1-2-0"] {
        let publication = fixture
            .publish_edition(PDF, Some(Edition::parse(edition)?))
            .await?;
        paths.push(publication.pdf().expect("published PDF edition"));
    }
    assert_eq!(
        paths
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "example-spec-pdf.pdf",
            "example-spec-pdf-pr-23.pdf",
            "example-spec-pdf-feature-x.pdf",
            "example-spec-pdf-1-2-0.pdf"
        ]
    );
    for path in &paths {
        assert!(
            path.is_file(),
            "{} was overwritten",
            path.display()
        );
    }
    let failure = fixture
        .publish_edition(PDF, None)
        .await
        .err()
        .expect("a PDF build without an edition input must fail");
    assert!(
        format!("{failure:#}").contains("explicit edition"),
        "{failure:#}"
    );
    Ok(())
}

/// Two PDF targets share one directory, each owning only its file; a second
/// target resolving to the same file name is a hard collision.
#[tokio::test]
async fn pdf_targets_share_a_directory_and_collide_on_equal_names() -> Result<()>
{
    let fixture = Fixture::new("pdf", "")?;
    fixture.publish(PDF).await?;
    let colliding = std::fs::read_to_string(&fixture.definitions)?.replace(
        "<urn:example:project> a ga:Project; ga:target <urn:example:spec>, \
         <urn:example:pdf> .",
        "<urn:example:project> a ga:Project; ga:target <urn:example:spec>, \
         <urn:example:pdf>, <urn:example:twin> .",
    );
    assert_ne!(
        colliding,
        std::fs::read_to_string(&fixture.definitions)?
    );
    let twin = colliding
        .split("<urn:example:pdf> a ga:Target")
        .nth(1)
        .unwrap();
    let twin = format!(
        "<urn:example:twin> a ga:Target{}",
        &twin[..twin.find(" .\n").unwrap() + 3]
    );
    std::fs::write(
        &fixture.definitions,
        format!("{colliding}\n{twin}"),
    )?;
    let failure = fixture
        .publish("urn:example:twin")
        .await
        .err()
        .expect("equal PDF file names must collide");
    assert!(
        format!("{failure:#}").contains("collision"),
        "{failure:#}"
    );
    Ok(())
}

/// The command line has no implicit edition: building a PDF target without
/// `--edition` fails before anything is published.
#[tokio::test]
async fn build_family_without_an_edition_fails_for_a_pdf_target() -> Result<()>
{
    let fixture = Fixture::new("pdf", "")?;
    let built = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "build-family", "--source"])
        .arg(&fixture.source)
        .arg("--definitions")
        .arg(&fixture.definitions)
        .args(["--project", "urn:example:project", "--target", PDF])
        .arg("--output-root")
        .arg(&fixture.output)
        .output()
        .await?;
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(!built.status.success());
    assert!(stderr.contains("explicit edition"), "{stderr}");
    assert!(!fixture.output.join("pdf").exists());
    Ok(())
}
