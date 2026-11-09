#[path = "../src/build_identity.rs"]
mod build_identity;
use build_identity::{CiBuild, Identity};
const REV: &str = "0123456789abcdef0123456789abcdef01234567";
fn ci(attempt: u64) -> CiBuild {
    CiBuild {
        repository:   "EKGF/grapharch".into(),
        workflow_ref: "EKGF/grapharch/.github/workflows/rust-ci.yml@refs/\
                       heads/main"
            .into(),
        revision:     REV.into(),
        run_id:       123456,
        run_number:   321,
        run_attempt:  attempt,
    }
}
#[test]
fn ci_build_numbers_are_distinct_from_canonical_release_versions() {
    let identity =
        Identity::new("0.2.2", Some(REV), false, None, Some(ci(1)), true)
            .unwrap();
    assert_eq!(identity.label, "0.2.2 (build 321)");
    assert_eq!(identity.number, "321");
    assert_eq!(identity.attempt, "1");
    assert_eq!(identity.channel, "ci");
    assert_eq!(identity.revision, REV);
    assert!(identity.details.contains(REV));
    assert!(identity.details.contains("refs/heads/main"));
    assert_eq!(
        identity.run_url,
        "https://github.com/EKGF/grapharch/actions/runs/123456/attempts/1"
    );
    let retry =
        Identity::new("0.2.2", Some(REV), false, None, Some(ci(2)), true)
            .unwrap();
    assert_eq!(retry.label, identity.label);
    assert_eq!(retry.number, identity.number);
    assert_eq!(retry.attempt, "2");
    assert!(retry.details.contains("producer attempt 2"));
    assert!(retry.run_url.ends_with("/attempts/2"));
    assert_ne!(retry.details, identity.details);
    let retained: CiBuild =
        serde_json::from_str(&serde_json::to_string(&ci(2)).unwrap()).unwrap();
    assert_eq!(retained.run_attempt, 2);
    let release = Identity::new(
        "0.2.2",
        Some(REV),
        false,
        Some("0.2.2"),
        Some(ci(1)),
        true,
    )
    .unwrap();
    assert_eq!(release.channel, "release");
    assert_eq!(release.label, "0.2.2 (build 321)");
}
#[test]
fn local_pinned_and_archive_builds_never_invent_ci_numbers() {
    for (pinned, kind) in [(false, "local"), (true, "source")] {
        let local =
            Identity::new("0.2.2", Some(REV), false, None, None, pinned)
                .unwrap();
        assert_eq!(
            local.label,
            format!("0.2.2 ({kind} 0123456789ab)")
        );
        assert_eq!(local.channel, kind);
        assert!(local.number.is_empty());
        assert!(local.attempt.is_empty());
        assert!(local.run_url.is_empty());
        let dirty = Identity::new("0.2.2", Some(REV), true, None, None, pinned)
            .unwrap();
        assert!(dirty.label.ends_with(" dirty)"));
    }
    let archive =
        Identity::new("0.2.2", None, false, None, None, false).unwrap();
    assert_eq!(archive.label, "0.2.2 (source archive)");
    assert_eq!(archive.channel, "source");
    assert!(archive.number.is_empty());
    assert!(archive.attempt.is_empty());
    assert!(archive.revision.is_empty());
    let release = Identity::new(
        "0.2.2",
        Some(REV),
        false,
        Some("0.2.2"),
        None,
        true,
    )
    .unwrap();
    assert_eq!(release.label, "0.2.2");
}
#[test]
fn claimed_ci_context_must_be_complete_clean_and_bound_to_the_exact_source() {
    for bad in [
        CiBuild { revision: "a".repeat(40), ..ci(1) },
        CiBuild { run_id: 0, ..ci(1) },
        CiBuild { run_number: 0, ..ci(1) },
        ci(0),
        CiBuild { repository: "../..".into(), ..ci(1) },
        CiBuild {
            workflow_ref: "other/repository/.github/workflows/rust-ci.yml@\
                           refs/heads/main"
                .into(),
            ..ci(1)
        },
    ] {
        assert!(
            Identity::new("0.2.2", Some(REV), false, None, Some(bad), true)
                .is_err()
        );
    }
    assert!(
        Identity::new("0.2.2", Some(REV), true, None, Some(ci(1)), true)
            .is_err()
    );
    assert!(
        Identity::new("0.2.2", None, false, None, Some(ci(1)), true).is_err()
    );
    assert!(serde_json::from_str::<CiBuild>("{\"run_number\":1}").is_err());
    let mut value = serde_json::to_value(ci(1)).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("secret".into(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<CiBuild>(value).is_err());
}
#[test]
fn release_claims_require_matching_version_and_clean_pinned_source() {
    assert!(
        Identity::new("0.2.2", None, false, Some("0.2.2"), None, true).is_err()
    );
    assert!(
        Identity::new(
            "0.2.2",
            Some(REV),
            true,
            Some("0.2.2"),
            None,
            true
        )
        .is_err()
    );
    assert!(
        Identity::new(
            "0.2.2",
            Some(REV),
            false,
            Some("0.2.1"),
            None,
            true
        )
        .is_err()
    );
    assert!(
        Identity::new("0.2.2", Some("bad"), false, None, None, false).is_err()
    );
}
