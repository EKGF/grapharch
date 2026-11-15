//! RDF renderer targets publish one file per declared RDF format from the
//! collected text of their sources, and hosting serves them by media type.
#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/rdf_targets.rs"]
mod rdf_targets;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

use {
    anyhow::{Result, ensure},
    grapharch::publication::hosting,
    oxrdf::Quad,
    oxrdfio::{JsonLdProfileSet, RdfFormat, RdfParser},
    rdf_targets::{TARGETS, build},
    revision_fixture::RevisionFixture,
    std::{collections::HashSet, path::Path},
};

/// Every published file and the media type it is served as.
const FILES: [(&str, &str); 7] = [
    ("dprod.ttl", "text/turtle"),
    ("dprod-all.ttl", "text/turtle"),
    ("dprod-all.nt", "application/n-triples"),
    ("dprod-all.nq", "application/n-quads"),
    ("dprod-all.trig", "application/trig"),
    ("dprod-all.jsonld", "application/ld+json"),
    ("dprod-all.rdf", "application/rdf+xml"),
];

fn parse(
    bytes: &[u8],
    format: RdfFormat,
    into: &mut HashSet<Quad>,
) -> Result<usize> {
    let mut count = 0;
    for quad in RdfParser::from_format(format)
        .with_base_iri("https://www.omg.org/spec/DPROD/")?
        .for_slice(bytes)
    {
        into.insert(quad?);
        count += 1;
    }
    Ok(count)
}

fn count(bytes: &[u8], media_type: &str) -> Result<usize> {
    let format = match RdfFormat::from_media_type(media_type) {
        Some(RdfFormat::JsonLd { .. }) => {
            RdfFormat::JsonLd { profile: JsonLdProfileSet::empty() }
        },
        Some(format) => format,
        None => anyhow::bail!("no parser for {media_type}"),
    };
    parse(bytes, format, &mut HashSet::new())
}

#[tokio::test]
async fn rdf_targets_publish_every_format_and_route_by_media_type() -> Result<()>
{
    let (output, temp) = build(TARGETS, &["--all-targets"]).await?;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let published = temp.path().join("published");
    let rdf = published.join("rdf");
    let source = std::fs::read("examples/dprod/content/ontology.ttl")?;
    assert_eq!(
        std::fs::read(rdf.join("dprod.ttl"))?,
        source,
        "the exact source text is the collected file, byte for byte"
    );
    let mut merged = HashSet::new();
    let ontology = parse(&source, RdfFormat::Turtle, &mut merged)?;
    let shapes = std::fs::read("examples/dprod/content/shapes.ttl")?;
    parse(&shapes, RdfFormat::Turtle, &mut merged)?;
    assert!(
        merged.len() > ontology,
        "the merged graph adds the shapes"
    );
    for (name, media_type) in FILES.iter().skip(1) {
        assert_eq!(
            count(&std::fs::read(rdf.join(name))?, media_type)?,
            merged.len(),
            "{name} serializes every merged statement"
        );
    }
    let mut listed = std::fs::read_dir(&rdf)?
        .map(|entry| Ok(entry?.file_name().into_string().unwrap()))
        .collect::<Result<Vec<_>>>()?;
    listed.sort();
    let mut expected = FILES
        .iter()
        .map(|(name, _)| name.to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        listed, expected,
        "a shared directory holds only the RDF files"
    );
    let mut manifests = String::new();
    for entry in std::fs::read_dir(published.join(".grapharch/targets"))? {
        manifests.push_str(&std::fs::read_to_string(entry?.path())?);
    }
    for (name, media_type) in FILES {
        assert!(
            manifests.contains(&format!(
                "name: {name}\n    media_type: {media_type}"
            )),
            "{name} is recorded as {media_type}: {manifests}"
        );
    }

    let site = temp.path().join("site");
    std::fs::create_dir(&site)?;
    std::fs::write(site.join("index.html"), "Home")?;
    let hosting = |platform: &str| -> Result<std::path::PathBuf> {
        let config = temp.path().join(format!("{platform}.yml"));
        std::fs::write(
            &config,
            format!(
                "version: 2\nplatform: {platform}\nmount: /dprod\nsite: \
                 site\nstatic_trees:\n  - source: published\n    destination: \
                 spec\n"
            ),
        )?;
        Ok(config)
    };
    let package_root = tempfile::tempdir()?;
    let package =
        hosting::package(&hosting("vercel")?, package_root.path(), None)
            .await?;
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package.join("config.json"))?)?;
    let routes = config["routes"].as_array().unwrap();
    for (name, media_type) in FILES {
        assert!(
            routes.iter().any(|route| {
                route["src"] ==
                    format!("^/dprod/spec/rdf/{}$", regex::escape(name)) &&
                    route["headers"]["Content-Type"] == media_type &&
                    route["continue"] == true
            }),
            "{name} is served as {media_type}"
        );
    }
    let negotiated = routes
        .iter()
        .filter(|route| route["src"] == "^/dprod/spec/ontology$")
        .collect::<Vec<_>>();
    assert_eq!(
        negotiated.len(),
        7,
        "six RDF variants and the HTML default"
    );
    for route in &negotiated {
        assert_eq!(route["headers"]["Vary"], "Accept");
    }
    let html = negotiated.last().unwrap();
    assert_eq!(
        html["dest"],
        "/dprod/spec/specification/index.html"
    );
    assert!(
        html.get("has").is_none(),
        "HTML is the unconditional default"
    );
    for (accept, expected) in [
        ("text/turtle", "/dprod/spec/rdf/dprod-all.ttl"),
        (
            "application/ld+json;q=0.9, */*;q=0.1",
            "/dprod/spec/rdf/dprod-all.jsonld",
        ),
        (
            "text/html,application/rdf+xml",
            "/dprod/spec/rdf/dprod-all.rdf",
        ),
        (
            "text/html, */*",
            "/dprod/spec/specification/index.html",
        ),
    ] {
        let served = negotiated
            .iter()
            .find(|route| {
                match route.get("has") {
                    None => true,
                    Some(has) => {
                        regex::Regex::new(has[0]["value"].as_str().unwrap())
                            .unwrap()
                            .is_match(accept)
                    },
                }
            })
            .unwrap();
        assert_eq!(served["dest"], expected, "Accept: {accept}");
        if expected.ends_with(".html") {
            continue;
        }
        assert_eq!(served["has"][0]["type"], "header");
        assert_eq!(served["has"][0]["key"], "accept");
    }
    let pages = hosting::package(
        &hosting("github-pages")?,
        tempfile::tempdir()?.path(),
        None,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        pages.contains("content negotiation of spec/ontology"),
        "a files-only platform cannot negotiate: {pages}"
    );
    Ok(())
}

#[tokio::test]
async fn rdf_targets_without_negotiation_package_for_files_only_hosting()
-> Result<()> {
    let targets = TARGETS.replace(
        "\n    ga:negotiatedPath \"ontology\" ; ga:negotiatedHtml \
         ex:specification .",
        " .",
    );
    let (output, temp) = build(&targets, &[
        "--target",
        "urn:dprod-showcase:rdf-source",
        "--target",
        "urn:dprod-showcase:rdf-all",
    ])
    .await?;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(temp.path().join("published/index.html"), "Home")?;
    let config = temp.path().join("pages.yml");
    std::fs::write(
        &config,
        "version: 2\nplatform: github-pages\nmount: /dprod\nsite: published\n",
    )?;
    let package =
        hosting::package(&config, &temp.path().join("deploy"), None).await?;
    assert_eq!(
        std::fs::read(package.join("dprod/rdf/dprod.ttl"))?,
        std::fs::read("examples/dprod/content/ontology.ttl")?
    );
    Ok(())
}

#[tokio::test]
async fn republishing_withdraws_formats_a_target_no_longer_declares()
-> Result<()> {
    use grapharch::publication::{
        render::{PublishOutcome, Publisher},
        revision_build::TargetSelection,
    };
    let mut fixture = RevisionFixture::new(Path::new("examples/dprod"))?;
    fixture.edit_definitions(|definitions| {
        Ok(format!("{definitions}\n{TARGETS}"))
    })?;
    let output = tempfile::tempdir()?;
    let selection = || {
        TargetSelection::targets(vec![oxrdf::NamedNode::new(
            "urn:dprod-showcase:rdf-all",
        )?])
    };
    let publish = async |fixture: &RevisionFixture| -> Result<()> {
        let (dataset, plans) = fixture.collect(selection()?).await?;
        let outcome = Publisher::new(dataset)
            .render(&plans[0], output.path())
            .await?;
        ensure!(
            matches!(outcome, PublishOutcome::Published(_)),
            "superseded"
        );
        Ok(())
    };
    publish(&fixture).await?;
    assert!(output.path().join("rdf/dprod-all.nq").is_file());
    fixture.edit_definitions(|definitions| {
        Ok(
            definitions
                .replace("ga:NQuadsFormat,\n        ga:TriGFormat, ", ""),
        )
    })?;
    publish(&fixture).await?;
    assert!(output.path().join("rdf/dprod-all.ttl").is_file());
    assert!(
        !output.path().join("rdf/dprod-all.nq").exists(),
        "N-Quads is withdrawn"
    );
    assert!(
        !output.path().join("rdf/dprod-all.trig").exists(),
        "TriG is withdrawn"
    );
    Ok(())
}
