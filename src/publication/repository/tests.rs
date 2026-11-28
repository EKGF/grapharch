//! Discovery over a real Git work tree and its `git archive` copy.
use {
    super::{
        DiagnosticCode,
        ForgeAccess,
        RepositoryDiscoverer,
        forge_api::ForgeCache,
    },
    std::{path::Path, process::Command},
};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    std::fs::write(
        r.join("Cargo.toml"),
        "[package]\nname = \"pets\"\nversion = \"0.3.0\"\ndescription = \"Pet \
         care\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    std::fs::write(r.join("README.md"), "# Pets\n").unwrap();
    git(r, &["init", "-q", "-b", "trunk"]);
    git(r, &["config", "user.name", "A"]);
    git(r, &["config", "user.email", "a@example.org"]);
    git(r, &[
        "remote",
        "add",
        "origin",
        "git@github.com:o/pets.git",
    ]);
    git(r, &["add", "."]);
    git(r, &["commit", "-q", "-m", "one"]);
    git(r, &["tag", "v0.2.0"]);
    git(r, &["commit", "-q", "--allow-empty", "-m", "two"]);
    git(r, &["tag", "v0.10.0"]);
    git(r, &["tag", "not-a-version"]);
    root
}

fn offline() -> RepositoryDiscoverer {
    RepositoryDiscoverer::with_environment(
        ForgeAccess::Offline,
        ForgeCache::disabled(),
        |_| None,
    )
}

#[tokio::test]
async fn a_git_work_tree_yields_commit_branch_tags_origin_and_manifest() {
    let root = repository();
    let facts = offline().discover(root.path()).await;
    let git = facts.git.as_ref().unwrap();
    assert_eq!(git.head.as_str(), git_head(root.path()));
    assert_eq!(
        git.default_branch.as_ref().unwrap().as_str(),
        "trunk"
    );
    assert_eq!(
        git.latest_tag.as_ref().unwrap().name.as_str(),
        "v0.10.0"
    );
    assert!(!git.dirty);
    assert!(git.first_commit <= git.last_commit);
    assert_eq!(facts.forge.as_ref().unwrap().path(), "o/pets");
    assert_eq!(facts.name().as_deref(), Some("pets"));
    assert_eq!(facts.version().unwrap().as_str(), "0.3.0");
    assert_eq!(facts.license().unwrap().as_str(), "MIT");
    assert_eq!(
        facts.diagnostics[0].code,
        DiagnosticCode::ForgeOffline
    );
    std::fs::write(root.path().join("README.md"), "# Changed\n").unwrap();
    assert!(offline().discover(root.path()).await.git.unwrap().dirty);
}

fn git_head(root: &Path) -> String { git(root, &["rev-parse", "HEAD"]) }

#[tokio::test]
async fn an_archive_copy_without_git_uses_the_manifest_repository() {
    let root = repository();
    let copy = tempfile::tempdir().unwrap();
    let archive = copy.path().join("a.tar");
    git(root.path(), &[
        "archive",
        "-o",
        archive.to_str().unwrap(),
        "HEAD",
    ]);
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(copy.path())
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::remove_file(&archive).unwrap();
    std::fs::write(copy.path().join("Cargo.toml"), "[package]\nname = \"pets\"\n\
        version = \"0.3.0\"\nrepository = \"https://github.com/o/pets\"\n").unwrap();
    let facts = offline().discover(copy.path()).await;
    assert!(facts.git.is_none());
    assert_eq!(facts.forge.as_ref().unwrap().path(), "o/pets");
}

#[tokio::test]
async fn an_unreachable_forge_degrades_to_git_metadata_with_a_diagnostic() {
    let root = repository();
    let discoverer = RepositoryDiscoverer::with_environment(
        ForgeAccess::Online,
        ForgeCache::disabled(),
        |name| {
            (name == "GITHUB_API_URL").then(|| "http://127.0.0.1:9/".to_owned())
        },
    );
    let facts = discoverer.discover(root.path()).await;
    assert!(facts.metadata.is_none());
    assert!(facts.git.is_some());
    assert_eq!(
        facts.diagnostics[0].code,
        DiagnosticCode::ForgeUnavailable
    );
}

#[tokio::test]
async fn an_invalid_manifest_is_a_diagnostic_not_a_failure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("package.json"), "{ not json").unwrap();
    let facts = offline().discover(root.path()).await;
    assert_eq!(
        facts.diagnostics[0].code,
        DiagnosticCode::ManifestInvalid
    );
}

#[tokio::test]
async fn a_release_tag_after_the_checked_out_commit_is_not_its_version() {
    let root = repository();
    let r = root.path();
    git(r, &[
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "three",
    ]);
    git(r, &["tag", "v0.11.0"]);
    git(r, &["checkout", "-q", "v0.10.0"]);
    let facts = offline().discover(r).await;
    assert_eq!(
        facts
            .git
            .as_ref()
            .unwrap()
            .latest_tag
            .as_ref()
            .unwrap()
            .name
            .as_str(),
        "v0.10.0"
    );
}
