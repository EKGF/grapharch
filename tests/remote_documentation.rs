use {
    anyhow::{Result, ensure},
    grapharch::publication::collect::git::remote_documentation,
    std::path::Path,
};

async fn git(root: &Path, args: &[&str]) -> Result<String> {
    let result = tokio::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .await?;
    ensure!(
        result.status.success(),
        "git fixture: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

#[tokio::test]
async fn documentation_repository_does_not_require_rdf_sources() -> Result<()> {
    let origin = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    git(origin.path(), &["init", "-b", "main"]).await?;
    git(origin.path(), &["config", "user.name", "Fixture"]).await?;
    git(origin.path(), &[
        "config",
        "user.email",
        "fixture@example.org",
    ])
    .await?;
    tokio::fs::create_dir(origin.path().join("docs")).await?;
    tokio::fs::write(
        origin.path().join("docs/index.md"),
        "# Original content\n",
    )
    .await?;
    tokio::fs::write(
        origin.path().join("mkdocs.yml"),
        "site_name: Original site\n",
    )
    .await?;
    tokio::fs::write(
        origin.path().join("LICENSE"),
        "Original source license\n",
    )
    .await?;
    git(origin.path(), &["add", "."]).await?;
    git(origin.path(), &[
        "commit",
        "-m",
        "Documentation source",
    ])
    .await?;
    let commit = git(origin.path(), &["rev-parse", "HEAD"]).await?;
    let url = url::Url::from_directory_path(origin.path())
        .unwrap()
        .to_string();
    let pinned =
        remote_documentation(&url, &commit, cache.path(), "urn:docs").await?;
    assert_eq!(pinned.commit, commit);
    assert!(pinned.files.is_empty());
    assert!(!pinned.root.join("docs/index.md").exists());
    Ok(())
}
