//! Links from authored pages to repository files that are not pages.
use {
    anyhow::{Result, ensure},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::{Path, PathBuf},
};

const PROJECT: &str = r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> . @prefix dct:<http://purl.org/dc/terms/> .
<urn:site:project> a ga:Project;dct:title "Example";ga:scope <urn:site:scope>;ga:target <urn:site:website> SETTING .
<urn:site:scope> a ga:Scope . <urn:site:profile> a ga:Profile .
<urn:site:website> a ga:Target;dct:title "Documentation";ga:scope <urn:site:scope>;ga:profile <urn:site:profile>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site" .
"#;
const SETTING: &str =
    "; ga:sourceLinkBase <https://github.com/example/documentation>";

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

/// A repository whose documentation lives in `docs/` and links to a
/// configuration file and a source file beside it.
async fn repository(root: &Path, setting: &str, page: &str) -> Result<String> {
    tokio::fs::create_dir_all(root.join("docs/guide")).await?;
    tokio::fs::create_dir_all(root.join(".config")).await?;
    tokio::fs::create_dir_all(root.join("src")).await?;
    tokio::fs::write(
        root.join("project.ttl"),
        PROJECT.replace("SETTING", setting),
    )
    .await?;
    tokio::fs::write(root.join(".config/environments.yml"), "a: 1\n").await?;
    tokio::fs::write(root.join("src/main.rs"), "fn main() {}\n").await?;
    tokio::fs::write(
        root.join("docs/README.md"),
        "# Home\n\nSee the [guide](guide/usage.md).\n",
    )
    .await?;
    tokio::fs::write(root.join("docs/guide/usage.md"), page).await?;
    git(root, &["init", "-b", "main"]).await?;
    git(root, &["config", "user.name", "Committer"]).await?;
    git(root, &["config", "user.email", "c@example.org"]).await?;
    git(root, &["add", "."]).await?;
    git(root, &["commit", "-m", "Initial"]).await?;
    git(root, &["rev-parse", "HEAD"]).await
}

async fn build(root: &Path) -> Result<PathBuf> {
    let built = website::collect(
        &root.join("docs"),
        &root.join("project.ttl"),
        "urn:site:project",
        "urn:site:website",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?.keep();
    let archive = output.join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, &output)
        .await?
    else {
        anyhow::bail!("publication required")
    };
    Ok(publication.directory)
}

const LINKING_PAGE: &str = "# Usage\n\nEdit [the \
                            environments](../../.config/environments.yml) and \
                            [the entry point](../../src/main.rs#L1).\n";

#[tokio::test]
async fn uncollected_repository_files_link_to_the_source_commit() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let commit = repository(source.path(), SETTING, LINKING_PAGE).await?;
    let site = build(source.path()).await?;
    let page =
        tokio::fs::read_to_string(site.join("guide/usage/index.html")).await?;
    let base =
        format!("https://github.com/example/documentation/blob/{commit}");
    assert!(
        page.contains(&format!("{base}/.config/environments.yml\"")),
        "{page}"
    );
    assert!(
        page.contains(&format!("{base}/src/main.rs#L1\"")),
        "{page}"
    );
    let home = tokio::fs::read_to_string(site.join("index.html")).await?;
    assert!(home.contains("href=\"guide/usage/\""), "{home}");
    Ok(())
}

#[tokio::test]
async fn uncollected_links_without_the_setting_name_it() -> Result<()> {
    let source = tempfile::tempdir()?;
    repository(source.path(), "", LINKING_PAGE).await?;
    let error = build(source.path()).await.expect_err("unresolved link");
    let message = format!("{error:#}");
    assert!(
        message.contains("unresolved documentation link"),
        "{message}"
    );
    assert!(message.contains("ga:sourceLinkBase"), "{message}");
    Ok(())
}

#[tokio::test]
async fn source_links_stay_inside_the_repository() -> Result<()> {
    let source = tempfile::tempdir()?;
    repository(
        source.path(),
        SETTING,
        "# Usage\n\nSee [outside](../../../outside.txt).\n",
    )
    .await?;
    let error = build(source.path()).await.expect_err("traversal rejected");
    let message = format!("{error:#}");
    assert!(
        message.contains("outside the repository"),
        "{message}"
    );
    Ok(())
}
