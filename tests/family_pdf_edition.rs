//! A family publishes its PDF edition from an explicit Typst target, without
//! a single-page HTML copy.
use {
    anyhow::Result,
    grapharch::publication::{
        ontology_family,
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
 ga:renderer ga:{renderer}; ga:destination "{destination}";
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
 rdfs:label "Example authority"@en; ga:shortName "Example" .
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
    assert!(files.iter().any(|file| file == "document.pdf"));
    assert!(
        !files.iter().any(|file| file.ends_with(".html")),
        "a Typst family target publishes no HTML copy: {files:?}"
    );
    let pdf = std::fs::read(publication.directory.join("document.pdf"))?;
    assert!(pdf.starts_with(b"%PDF"));
    let typst =
        std::fs::read_to_string(publication.directory.join("main.typ"))?;
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
