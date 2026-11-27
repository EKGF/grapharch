use {
    anyhow::{Result, ensure},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    },
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

/// A small code repository: a README, a guide, a license, and an agent
/// instruction file that the showcase does not select.
async fn repository(root: &Path) -> Result<String> {
    git(root, &["init", "-b", "main"]).await?;
    git(root, &["config", "user.name", "Original author"]).await?;
    git(root, &[
        "config",
        "user.email",
        "author@example.org",
    ])
    .await?;
    tokio::fs::create_dir(root.join("docs")).await?;
    tokio::fs::write(
        root.join("README.md"),
        "# example-crate\n\nA library.\n\nSee the [guide](docs/guide.md) and \
         the [Apache license](LICENSE-APACHE).\n",
    )
    .await?;
    tokio::fs::write(
        root.join("docs/guide.md"),
        "# Guide\n\nUse the [library](../README.md).\n",
    )
    .await?;
    tokio::fs::write(
        root.join("LICENSE"),
        "Copyright original authors. Redistribution permitted.\n",
    )
    .await?;
    tokio::fs::write(
        root.join("LICENSE-APACHE"),
        "Apache License, Version 2.0.\n",
    )
    .await?;
    tokio::fs::write(
        root.join("CLAUDE.md"),
        "# Agent instructions\n\nNot documentation.\n",
    )
    .await?;
    git(root, &["add", "."]).await?;
    git(root, &["commit", "-m", "Code repository fixture"]).await?;
    git(root, &["rev-parse", "HEAD"]).await
}

async fn showcase(
    directory: &Path,
    url: &url::Url,
    commit: &str,
    paths: &str,
    licenses: &str,
) -> Result<std::path::PathBuf> {
    let path = directory.join("showcase.ttl");
    tokio::fs::write(
        &path,
        format!(
            "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
             .\n<urn:grapharch:showcase:example> a ga:Showcase ; \
             ga:gitRepository \"{url}\" ; ga:gitCommit \"{commit}\" ; \
             ga:materializedPath {paths} ; ga:licensePath {licenses} .\n"
        ),
    )
    .await?;
    Ok(path)
}

#[tokio::test]
async fn pinned_code_repository_publishes_only_selected_paths() -> Result<()> {
    let source = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let commit = repository(source.path()).await?;
    let url = url::Url::from_directory_path(source.path()).unwrap();
    let configuration = showcase(
        output.path(),
        &url,
        &commit,
        "\"README.md\", \"docs\", \"LICENSE\", \"LICENSE-APACHE\"",
        "\"LICENSE\", \"LICENSE-APACHE\"",
    )
    .await?;
    let (dataset, plans) =
        showcases::repository(&configuration, cache.path()).await?;
    assert_eq!(plans.len(), 1);
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let site_root = output.path().join("site-root");
    let PublishOutcome::Published(site) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&plans[0], &site_root)
        .await?
    else {
        panic!("expected repository publication");
    };
    let home =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(home.contains("example-crate"));
    assert!(home.contains("A library."));
    assert!(
        tokio::fs::read_to_string(site.directory.join("docs/guide/index.html"))
            .await?
            .contains("Use the")
    );
    assert!(
        !site.directory.join("CLAUDE/index.html").exists(),
        "unselected repository files are never published"
    );
    assert!(
        tokio::fs::read_to_string(site.directory.join("LICENSE"))
            .await?
            .contains("Redistribution permitted")
    );
    assert!(
        tokio::fs::read_to_string(site.directory.join("LICENSE-APACHE"))
            .await?
            .contains("Apache License"),
        "every selected license is a downloadable asset"
    );
    assert!(
        tokio::fs::read_to_string(site.directory.join("provenance.html"))
            .await?
            .contains(&commit)
    );
    Ok(())
}

#[tokio::test]
async fn repository_showcase_requires_its_license_to_be_selected() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let commit = repository(source.path()).await?;
    let url = url::Url::from_directory_path(source.path()).unwrap();
    let configuration = showcase(
        output.path(),
        &url,
        &commit,
        "\"README.md\"",
        "\"LICENSE\"",
    )
    .await?;
    let error = showcases::repository(&configuration, cache.path())
        .await
        .err()
        .expect("an unselected license is rejected");
    assert!(
        error.to_string().contains("ga:licensePath"),
        "{error}"
    );
    Ok(())
}

#[tokio::test]
async fn repository_showcase_rejects_a_movable_reference() -> Result<()> {
    let source = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    repository(source.path()).await?;
    let url = url::Url::from_directory_path(source.path()).unwrap();
    let configuration = showcase(
        output.path(),
        &url,
        "main",
        "\"README.md\", \"LICENSE\"",
        "\"LICENSE\"",
    )
    .await?;
    let error = showcases::repository(&configuration, cache.path())
        .await
        .err()
        .expect("a branch name is not an immutable revision");
    assert!(error.to_string().contains("immutable"), "{error}");
    Ok(())
}
