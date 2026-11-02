use super::api::{Deployment, PullRequest, discover};

#[test]
fn open_same_repository_previews_are_advertised_but_old_routes_survive() {
    let deployments = serde_json::from_value::<Vec<Deployment>>(serde_json::json!([
        {"url":"old.vercel.app","created":1,"meta":{"githubCommitRef":"feature/new"}},
        {"url":"new.vercel.app","created":2,"meta":{"githubCommitRef":"feature/new"}},
        {"url":"merged.vercel.app","created":2,"meta":{"githubCommitRef":"feature/merged"}},
        {"url":"bot.vercel.app","created":2,"meta":{"githubCommitRef":"dependabot/a"}},
        {"url":"fork.vercel.app","created":2,"meta":{"githubCommitRef":"fork"}}
    ])).unwrap();
    let pulls = serde_json::from_value::<Vec<PullRequest>>(serde_json::json!([
        {"head":{"ref":"feature/new","repo":{"full_name":"EKGF/dprod"}}},
        {"head":{"ref":"dependabot/a","repo":{"full_name":"EKGF/dprod"}}},
        {"head":{"ref":"fork","repo":{"full_name":"someone/dprod"}}}
    ]))
    .unwrap();
    let versions = discover(
        deployments,
        pulls,
        "EKGF/dprod",
        "dprod/spec/",
        &["dependabot/".into()],
        &[],
    )
    .unwrap();
    assert_eq!(versions.len(), 4);
    let advertised: Vec<_> = versions.iter().filter(|v| v.advertised).collect();
    assert_eq!(advertised.len(), 1);
    assert_eq!(
        advertised[0].url,
        "https://new.vercel.app/dprod/spec/"
    );
    assert_eq!(advertised[0].slug, "feature-new");
}

#[test]
fn colliding_branch_slugs_fail_instead_of_overwriting_routes() {
    let deployments = serde_json::from_value::<Vec<Deployment>>(serde_json::json!([
        {"url":"one.vercel.app","created":1,"meta":{"githubCommitRef":"a/b"}},
        {"url":"two.vercel.app","created":2,"meta":{"githubCommitRef":"a-b"}}
    ])).unwrap();
    assert!(
        discover(
            deployments,
            vec![],
            "EKGF/dprod",
            "spec/",
            &[],
            &[]
        )
        .is_err()
    );
}
