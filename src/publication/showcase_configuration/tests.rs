use super::*;

const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
                        .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n";

async fn read(body: &str) -> Result<ShowcaseConfiguration> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("showcase.ttl");
    std::fs::write(&path, format!("{PREFIXES}{body}"))?;
    let configuration = ShowcaseConfiguration::read(&path).await;
    drop(directory);
    configuration
}

#[tokio::test]
async fn one_showcase_supplies_typed_settings() -> Result<()> {
    let configuration = read(
        "<urn:case> a ga:Showcase ; ga:gitRepository \"https://example.org/r.git\" \
         ; ga:allTargets true ; ga:showcaseTargets ( <urn:b> <urn:a> ) ; \
         ga:materializedPath \"docs\", \"LICENSE\" .",
    )
    .await?;
    assert_eq!(configuration.id().as_str(), "urn:case");
    assert_eq!(
        configuration.text("gitRepository")?,
        "https://example.org/r.git"
    );
    assert_eq!(configuration.flag("allTargets")?, Some(true));
    assert_eq!(
        configuration
            .node_list("showcaseTargets")?
            .unwrap()
            .iter()
            .map(NamedNode::as_str)
            .collect::<Vec<_>>(),
        ["urn:b", "urn:a"],
        "list settings keep their authored order"
    );
    assert_eq!(configuration.texts("materializedPath")?, [
        "LICENSE", "docs"
    ]);
    configuration.accept_only("test", &[
        "gitRepository",
        "allTargets",
        "showcaseTargets",
        "materializedPath",
    ])?;
    let error = configuration
        .accept_only("test", &["gitRepository"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("does not support ga:"), "{error}");
    Ok(())
}

#[tokio::test]
async fn configurations_describe_exactly_one_identified_showcase() -> Result<()>
{
    for (body, expected) in [
        (
            "<urn:other> ga:gitCommit \"x\" .",
            "declares no ga:Showcase",
        ),
        (
            "<urn:a> a ga:Showcase . <urn:b> a ga:Showcase .",
            "more than one ga:Showcase",
        ),
        ("[] a ga:Showcase .", "stable IRI"),
    ] {
        let error = match read(body).await {
            Ok(_) => bail!("accepted {body}"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(expected), "{body}: {error}");
    }
    let configuration = read(
        "<urn:case> a ga:Showcase ; ga:gitCommit \"a\", \"b\" ; ga:allTargets \
         \"yes\" .",
    )
    .await?;
    assert!(
        configuration
            .text("gitCommit")
            .unwrap_err()
            .to_string()
            .contains("conflicting ga:gitCommit")
    );
    assert!(
        configuration
            .flag("allTargets")
            .unwrap_err()
            .to_string()
            .contains("xsd:boolean")
    );
    assert!(
        configuration
            .commit("gitCommit", "showcase commit")
            .is_err()
    );
    Ok(())
}
