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
