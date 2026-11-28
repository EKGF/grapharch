use {
    super::{SourceBudget, SourceWalk},
    crate::publication::website::exclusions::SourcePattern,
    std::path::Path,
};

fn markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == "md" || ext == "ttl")
}

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn names(root: &Path, files: Vec<std::path::PathBuf>) -> Vec<String> {
    files
        .into_iter()
        .map(|path| {
            path.strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/")
        })
        .collect()
}

/// A `git archive` copy: ignore files but no `.git` directory.
fn archive() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    write(r, "README.md", "# Home");
    write(r, "docs/guide.md", "# Guide");
    write(r, ".gitignore", "generated/\n*.draft.md\n");
    write(r, "generated/api.md", "# Generated");
    write(r, "docs/notes.draft.md", "# Draft");
    write(r, ".grapharchignore", "docs/internal/\n");
    write(r, "docs/internal/secret.md", "# Internal");
    write(r, ".ignore", "scratch.md\n");
    write(r, "scratch.md", "# Scratch");
    write(r, "CLAUDE.md", "# Agent");
    write(r, "crates/a/AGENTS.md", "# Agent");
    write(r, "PLAN.md", "# Plan");
    write(r, "NPM_PUBLISH_STATUS.md", "# Status");
    write(r, ".github/workflows/readme.md", "# CI");
    write(r, "tests/fixtures/broken.ttl", "not RDF");
    write(r, "tests/data/case.ttl", "not RDF");
    write(r, "tests/README.md", "# How to test");
    write(r, "src/__snapshots__/a.md", "# Snapshot");
    write(r, "testdata/x.md", "# Data");
    write(r, "node_modules/p/README.md", "# Dependency");
    write(r, "_layouts/x.md", "# Layout");
    root
}

#[tokio::test]
async fn archive_copies_honour_ignore_files_and_default_exclusions() {
    let root = archive();
    let files = SourceWalk::new(root.path(), markdown)
        .files()
        .await
        .unwrap();
    assert_eq!(names(root.path(), files), [
        "README.md",
        "docs/guide.md"
    ]);
}

#[tokio::test]
async fn inclusions_bring_default_exclusions_back() {
    let root = archive();
    let files = SourceWalk::new(root.path(), markdown)
        .with_inclusions(vec![
            SourcePattern::parse("**/AGENTS.md").unwrap(),
            SourcePattern::parse("tests/**").unwrap(),
        ])
        .unwrap()
        .files()
        .await
        .unwrap();
    let names = names(root.path(), files);
    assert!(
        names.contains(&"crates/a/AGENTS.md".to_owned()),
        "{names:?}"
    );
    assert!(names.contains(&"tests/fixtures/broken.ttl".to_owned()));
    assert!(!names.contains(&"CLAUDE.md".to_owned()));
    // Ignore files still apply: inclusions override only the defaults.
    assert!(!names.contains(&"generated/api.md".to_owned()));
}

#[tokio::test]
async fn the_budget_is_typed_and_names_its_setting() {
    let root = archive();
    let error = SourceWalk::new(root.path(), markdown)
        .with_budget(SourceBudget::try_new(1).unwrap())
        .files()
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("ga:maxDiscoveredSources"),
        "{error}"
    );
    assert!(error.contains("--max-sources"), "{error}");
    assert!(SourceBudget::try_new(0).is_err());
}

#[tokio::test]
async fn large_trees_are_not_capped_at_256_sources() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..600 {
        write(
            root.path(),
            &format!("d{}/p{index}.md", index % 7),
            "# P",
        );
    }
    let files = SourceWalk::new(root.path(), markdown)
        .files()
        .await
        .unwrap();
    assert_eq!(files.len(), 600);
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_are_not_followed() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "README.md", "# Home");
    std::os::unix::fs::symlink(
        root.path().join("README.md"),
        root.path().join("LINK.md"),
    )
    .unwrap();
    let files = SourceWalk::new(root.path(), markdown)
        .files()
        .await
        .unwrap();
    assert_eq!(names(root.path(), files), ["README.md"]);
}
