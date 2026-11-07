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
    tokio::fs::write(
        origin.path().join("docs/index.md"),
        "# Changed content\n",
    )
    .await?;
    git(origin.path(), &[
        "commit",
        "-am",
        "Changed documentation",
    ])
    .await?;
    let _newer =
        remote_documentation(&url, "main", cache.path(), "urn:docs").await?;
    let output = tempfile::tempdir()?;
    let selected =
        ["docs", "mkdocs.yml", "LICENSE"].map(std::path::PathBuf::from);
    let blobs = pinned
        .materialize_documentation(&selected, output.path())
        .await?;
    assert_eq!(blobs.len(), 3);
    assert_eq!(
        tokio::fs::read_to_string(output.path().join("docs/index.md")).await?,
        "# Original content\n"
    );
    assert_eq!(
        tokio::fs::read_to_string(output.path().join("LICENSE")).await?,
        "Original source license\n"
    );
    let escape = tempfile::tempdir()?;
    assert!(
        pinned
            .materialize_documentation(&["../LICENSE".into()], escape.path())
            .await
            .is_err()
    );
    #[cfg(unix)]
    {
        tokio::fs::symlink("../LICENSE", origin.path().join("docs/link.md"))
            .await?;
        git(origin.path(), &["add", "."]).await?;
        git(origin.path(), &["commit", "-m", "Add symlink"]).await?;
        let unsafe_tree =
            remote_documentation(&url, "main", cache.path(), "urn:docs")
                .await?;
        let destination = tempfile::tempdir()?;
        assert!(
            unsafe_tree
                .materialize_documentation(&selected, destination.path())
                .await
                .is_err()
        );
    }

    Ok(())
}
