//! A target's RDF downloads are built from the exact collected source text.
#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

use {
    anyhow::{Result, ensure},
    oxrdfio::{JsonLdProfileSet, RdfFormat, RdfParser},
    revision_fixture::RevisionFixture,
    std::path::Path,
};

/// Downloads declared on the specification target of the DPROD fixture.
const DOWNLOADS: &str = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:dprod-showcase:> .
ex:specification ga:download ex:download:ttl, ex:download:jsonld, ex:download:rdf, ex:download:all .
ex:download:ttl a ga:Download ; ga:fileName "dprod.ttl" ;
    ga:downloadFormat ga:SourceFileFormat ; ga:downloadSource ex:collector:ontology .
ex:download:jsonld a ga:Download ; ga:fileName "dprod.jsonld" ;
    ga:downloadFormat ga:JsonLdFormat ; ga:downloadSource ex:collector:ontology .
ex:download:rdf a ga:Download ; ga:fileName "dprod.rdf" ;
    ga:downloadFormat ga:RdfXmlFormat ; ga:downloadSource ex:collector:ontology .
ex:download:all a ga:Download ; ga:fileName "dprod-all.ttl" ;
    ga:downloadFormat ga:TurtleFormat ;
    ga:downloadSource ex:collector:ontology, ex:collector:shapes .
"#;

async fn build(
    extra: &str,
) -> Result<(std::process::Output, tempfile::TempDir)> {
    let mut fixture = RevisionFixture::new(Path::new("examples/dprod"))?;
    fixture.edit_definitions(|definitions| {
        Ok(format!("{definitions}\n{extra}"))
    })?;
    let output = tempfile::tempdir()?;
    let root = output.path().join("published");
    let result = fixture.build(&root, &["--all-targets"]).await?;
    Ok((result, output))
}

fn graph(bytes: &[u8], format: RdfFormat) -> Result<usize> {
    let mut count = 0;
    for quad in RdfParser::from_format(format)
        .with_base_iri("https://example.org/")?
        .for_slice(bytes)
    {
        quad?;
        count += 1;
    }
    Ok(count)
}

#[tokio::test]
async fn declared_downloads_publish_source_bytes_and_serializations()
-> Result<()> {
    let (output, temp) = build(DOWNLOADS).await?;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let specification = temp.path().join("published/specification");
    let source = std::fs::read("examples/dprod/content/ontology.ttl")?;
    assert_eq!(
        std::fs::read(specification.join("dprod.ttl"))?,
        source,
        "the source download is the collected file, byte for byte"
    );
    let triples = graph(&source, RdfFormat::Turtle)?;
    assert_eq!(
        graph(
            &std::fs::read(specification.join("dprod.jsonld"))?,
            RdfFormat::JsonLd { profile: JsonLdProfileSet::empty() }
        )?,
        triples
    );
    assert_eq!(
        graph(
            &std::fs::read(specification.join("dprod.rdf"))?,
            RdfFormat::RdfXml
        )?,
        triples
    );
    let shapes = graph(
        &std::fs::read("examples/dprod/content/shapes.ttl")?,
        RdfFormat::Turtle,
    )?;
    let all = graph(
        &std::fs::read(specification.join("dprod-all.ttl"))?,
        RdfFormat::Turtle,
    )?;
    assert!(
        all > triples && all <= triples + shapes,
        "the combined graph merges both sources"
    );
    let manifest =
        std::fs::read_to_string(specification.join("manifest.yaml"))?;
    for name in ["dprod.ttl", "dprod.jsonld", "dprod.rdf", "dprod-all.ttl"] {
        assert!(
            manifest.contains(name),
            "{name} is a published file"
        );
    }
    Ok(())
}

#[tokio::test]
async fn invalid_downloads_fail_the_build() -> Result<()> {
    for (definitions, message) in [
        (
            DOWNLOADS.replace(
                "ga:JsonLdFormat ; ga:downloadSource ex:collector:ontology",
                "ga:SourceFileFormat ; ga:downloadSource \
                 ex:collector:ontology, ex:collector:shapes",
            ),
            "needs exactly one ga:downloadSource",
        ),
        (
            DOWNLOADS.replace("\"dprod.rdf\"", "\"../dprod.rdf\""),
            "ga:fileName must be one file name",
        ),
        (
            DOWNLOADS.replace("\"dprod.rdf\"", "\"dprod.ttl\""),
            "more than once",
        ),
        (
            DOWNLOADS.replace("\"dprod.rdf\"", "\"index.html\""),
            "collides with a generated publication file",
        ),
        (
            DOWNLOADS.replace(
                "ga:downloadSource ex:collector:ontology .\nex:download:rdf",
                "ga:downloadSource ex:abstract .\nex:download:rdf",
            ),
            "must be a ga:RdfFileCollector use",
        ),
    ] {
        let (output, _temp) = build(&definitions).await?;
        assert!(!output.status.success(), "{message}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{message}: {stderr}");
    }
    Ok(())
}
