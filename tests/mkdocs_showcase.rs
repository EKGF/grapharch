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

#[tokio::test]
async fn remote_mkdocs_showcase_retains_source_license_images_and_git_revision()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    git(source.path(), &["init", "-b", "main"]).await?;
    git(source.path(), &[
        "config",
        "user.name",
        "Original author",
    ])
    .await?;
    git(source.path(), &[
        "config",
        "user.email",
        "author@example.org",
    ])
    .await?;
    tokio::fs::create_dir(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("mkdocs.yml"),
        "site_name: Independent documentation\nnav:\n  - Home: index.md\n  - \
         Details: details.md\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "title: Original home\n\n# Independent \
         documentation\n\n[Details](details.md).\n\n![Original \
         diagram](diagram.svg)\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/details.md"),
        "# Details\n\nOriginal source prose.\n",
    )
    .await?;
    tokio::fs::write(source.path().join("docs/diagram.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 40"><rect width="100" height="40" fill="blue"/></svg>"#).await?;
    tokio::fs::write(
        source.path().join("LICENSE"),
        "Copyright Original author\nRedistribution permitted with this \
         notice.\n",
    )
    .await?;
    git(source.path(), &["add", "."]).await?;
    git(source.path(), &[
        "commit",
        "-m",
        "Original documentation",
    ])
    .await?;
    let commit = git(source.path(), &["rev-parse", "HEAD"]).await?;
    let repository = url::Url::from_directory_path(source.path())
        .unwrap()
        .to_string();
    let configuration = output.path().join("showcase.yaml");
    tokio::fs::write(
        &configuration,
        format!(
            "repository: {repository}\ncommit: {commit}\nconfiguration: \
             mkdocs.yml\npaths: [docs, mkdocs.yml, LICENSE]\nlicense: \
             LICENSE\n"
        ),
    )
    .await?;
    let (dataset, plans) =
        showcases::mkdocs(&configuration, cache.path()).await?;
    assert_eq!(plans.len(), 1);
    let (_, repeated) = showcases::mkdocs(&configuration, cache.path()).await?;
    assert_eq!(
        plans[0].id, repeated[0].id,
        "temporary materialization locations must not change publication \
         identity"
    );
    let archive = output.path().join("retained.nq");
    dataset.export(archive.clone()).await?;
    let graph = tokio::fs::read_to_string(&archive).await?;
    assert!(graph.contains(&commit));
    assert!(graph.contains("Original author"));
    assert!(graph.contains("Redistribution permitted with this notice."));
    assert!(graph.contains("git-file"));
    tokio::fs::remove_dir_all(source.path()).await?;
    tokio::fs::remove_dir_all(cache.path()).await?;
    tokio::fs::remove_file(configuration).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("expected publication")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(html.contains("Independent documentation"));
    assert!(html.contains("details/"));
    assert!(html.contains("Original diagram"));
    assert!(!html.contains("title: Original home"));
    assert!(
        tokio::fs::read_to_string(site.directory.join("LICENSE"))
            .await?
            .contains("Redistribution permitted")
    );
    assert!(
        tokio::fs::read_to_string(site.directory.join("provenance.html"))
            .await?
            .contains(&commit)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "fetches the explicitly pinned independent public documentation"]
async fn public_bootswatch_documentation_is_retained_without_mkdocs_runtime()
-> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = root.join("output/mkdocs-pinned-review");
    tokio::fs::create_dir_all(&output).await?;
    let (dataset, plans) = showcases::mkdocs(
        &root.join("examples/mkdocs-bootswatch/showcase.yaml"),
        &root.join("output/cache/git"),
    )
    .await?;
    let archive = output.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(site) =
        Publisher::new(restored).render(&plans[0], &output).await?
    else {
        panic!("expected public documentation publication");
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(html.contains("MkDocs Bootswatch"));
    assert!(
        tokio::fs::read_to_string(site.directory.join("LICENSE"))
            .await?
            .contains("Redistribution")
    );
    assert!(
        tokio::fs::read_to_string(site.directory.join("provenance.html"))
            .await?
            .contains("374197dbc3b89b830075b7d67c66c9407968855a")
    );
    println!("{}", site.directory.display());
    Ok(())
}
