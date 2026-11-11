use super::validate_artifact_count;

#[test]
fn multi_edition_gallery_retains_more_than_128_outputs_with_a_finite_budget() {
    assert!(validate_artifact_count(129).is_ok());
    assert!(validate_artifact_count(512).is_ok());
    assert!(validate_artifact_count(513).is_err());
}

#[test]
fn html_only_showcase_does_not_require_a_pdf() -> anyhow::Result<()> {
    let case = oxrdf::NamedNode::new("urn:case")?;
    let artifacts = std::collections::HashMap::from([(
        "site/index.html".to_owned(),
        oxrdf::NamedNode::new("urn:html")?,
    )]);
    assert!(
        super::artifact_selection(&artifacts, None, "document.pdf", &case)
            .is_ok(),
        "absence of an unrequested PDF is valid"
    );
    assert!(
        super::artifact_selection(
            &artifacts,
            Some("review/document.pdf"),
            "document.pdf",
            &case
        )
        .is_err(),
        "an explicitly requested missing PDF is an error"
    );
    Ok(())
}

/// The gallery, not the showcased definitions, supplies where an
/// exact-revision showcase is deployed: its artifacts location and slug.
#[test]
fn revision_showcases_are_deployed_beneath_the_gallery_artifacts_location()
-> anyhow::Result<()> {
    let project = oxrdf::NamedNode::new("urn:docs")?;
    let case = "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
                .\n<urn:docs> ga:showcase <urn:case> .\n<urn:case> a \
                ga:Showcase ; ga:slug \"dprod\" ; ga:source \
                \"dprod/live.yaml\" ; ga:showcaseRunner \
                ga:RevisionBuildShowcaseRunner .\n";
    let cases = super::read(
        format!(
            "{case}<urn:docs> ga:showcaseArtifactsUrl \
             <https://example.org/docs/showcases/artifacts/> .\n"
        )
        .as_bytes(),
        &project,
    )?;
    assert_eq!(
        cases[0].publication.as_ref().map(|url| url.as_str()),
        Some("https://example.org/docs/showcases/artifacts/dprod/")
    );
    let error = match super::read(case.as_bytes(), &project) {
        Ok(_) => anyhow::bail!("accepted a revision showcase without location"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("requires ga:showcaseArtifactsUrl"),
        "{error}"
    );
    let error = match super::read(
        format!(
            "{case}<urn:docs> ga:showcaseArtifactsUrl \
             <https://example.org/docs/showcases/artifacts> .\n"
        )
        .as_bytes(),
        &project,
    ) {
        Ok(_) => anyhow::bail!("accepted a non-directory location"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("absolute HTTP(S) directory URL"),
        "{error}"
    );
    Ok(())
}
