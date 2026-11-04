use super::validate_artifact_count;

#[test]
fn multi_edition_gallery_retains_more_than_128_outputs_with_a_finite_budget() {
    assert!(validate_artifact_count(129).is_ok());
    assert!(validate_artifact_count(512).is_ok());
    assert!(validate_artifact_count(513).is_err());
}
