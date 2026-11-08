#[path = "../src/build_identity.rs"]
mod build_identity;
use build_identity::Identity;
const REV: &str = "0123456789abcdef0123456789abcdef01234567";
#[test]
fn development_release_dirty_and_source_builds_are_distinct() {
    let dev = Identity::new("0.2.2", Some(REV), false, None).unwrap();
    assert_eq!(dev.label, "0.2.2-dev+0123456789ab");
    assert_eq!(dev.channel, "development");
    assert_eq!(dev.revision, REV);
    assert!(
        Identity::new("0.2.2", Some(REV), true, None)
            .unwrap()
            .label
            .ends_with(".dirty")
    );
    let release =
        Identity::new("0.2.2", Some(REV), false, Some("0.2.2")).unwrap();
    assert_eq!(release.label, "0.2.2");
    assert_eq!(release.channel, "release");
    let source = Identity::new("0.2.2", None, false, None).unwrap();
    assert_eq!(source.label, "0.2.2-source");
    assert_eq!(source.channel, "source");
}
#[test]
fn release_claims_require_matching_version_and_clean_pinned_source() {
    assert!(Identity::new("0.2.2", None, false, Some("0.2.2")).is_err());
    assert!(Identity::new("0.2.2", Some(REV), true, Some("0.2.2")).is_err());
    assert!(Identity::new("0.2.2", Some(REV), false, Some("0.2.1")).is_err());
    assert!(Identity::new("0.2.2", Some("bad"), false, None).is_err());
}
