use {
    anyhow::{Result, ensure},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::Path,
};

async fn git(path: &Path, args: &[&str]) -> Result<String> {
    let result = tokio::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .await?;
    ensure!(
        result.status.success(),
        "fixture git failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?.trim().into())
}
async fn fixture(root: &Path) -> Result<()> {
    tokio::fs::create_dir_all(root.join("docs")).await?;
    tokio::fs::write(root.join("project.ttl"), r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:site:project> a ga:Project;dct:title "Example documentation";ga:scope <urn:site:scope>;ga:target <urn:site:website> .
<urn:site:scope> a ga:Scope . <urn:site:profile> a ga:Profile .
<urn:site:website> a ga:Target;dct:title "Documentation";ga:scope <urn:site:scope>;ga:profile <urn:site:profile>;ga:renderer "website";ga:destination "site" .
"#).await?;
    tokio::fs::write(
        root.join("docs/index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:site:home\n---\n# Home\n\nWorking tree documentation.\n",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn website_collection_pins_enclosing_git_repository_and_recent_committers()
-> Result<()> {
    let source = tempfile::tempdir()?;
    fixture(source.path()).await?;
    git(source.path(), &["init", "-b", "main"]).await?;
    git(source.path(), &[
        "config",
        "user.name",
        "Site Committer",
    ])
    .await?;
    git(source.path(), &[
        "config",
        "user.email",
        "private@example.org",
    ])
    .await?;
    git(source.path(), &[
        "remote",
        "add",
        "origin",
        "https://github.com/example/documentation.git",
    ])
    .await?;
    git(source.path(), &["add", "."]).await?;
    git(source.path(), &[
        "commit",
        "-m",
        "Initial documentation",
    ])
    .await?;
    let commit = git(source.path(), &["rev-parse", "HEAD"]).await?;
    git(source.path(), &["tag", "docs-preview"]).await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:site:home\n---\n# Home\n\nUncommitted documentation update.\n",
    )
    .await?;
    let built = website::collect(
        &source.path().join("docs"),
        &source.path().join("project.ttl"),
        "urn:site:project",
        "urn:site:website",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    source.close()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let page =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(page.contains("Uncommitted documentation update."));
    assert!(page.contains("href=\"provenance.html\""));
    let provenance = tokio::fs::read_to_string(
        publication.directory.join("provenance.html"),
    )
    .await?;
    assert!(
        provenance.contains(&commit),
        "source SHA survives export/restore and clone deletion"
    );
    assert!(
        provenance.contains("https://github.com/example/documentation.git")
    );
    assert!(
        provenance.contains("Site Committer") &&
            provenance.contains("docs-preview")
    );
    assert!(provenance.contains("uncommitted changes"));
    assert!(!provenance.contains("private@example.org"));
    assert_eq!(
        provenance.matches("<dt>Repository</dt>").count(),
        1,
        "source and definitions in same clone must deduplicate provenance"
    );
    Ok(())
}

#[tokio::test]
async fn malformed_enclosing_git_metadata_is_an_error_not_missing_provenance()
-> Result<()> {
    let source = tempfile::tempdir()?;
    fixture(source.path()).await?;
    tokio::fs::write(
        source.path().join(".git"),
        "malformed Git worktree metadata",
    )
    .await?;
    let result = website::collect(
        &source.path().join("docs"),
        &source.path().join("project.ttl"),
        "urn:site:project",
        "urn:site:website",
        2,
    )
    .await;
    assert!(
        result.is_err(),
        "a broken clone must not silently become a non-Git source"
    );
    Ok(())
}
