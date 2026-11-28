//! Zero-configuration builds of code repositories: an archive copy without
//! `.git` selects sources through its ignore files and default exclusions and
//! takes its identity from the package manifest.
use {anyhow::Result, std::path::Path, tokio::process::Command};

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// A `git archive`-style copy of a Rust workspace.
fn archive_copy(root: &Path) {
    write(root, "Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n\
        [workspace.package]\nversion = \"3.1.4\"\nlicense = \"MIT\"\n\
        repository = \"https://github.com/example/pets\"\n");
    write(
        root,
        "crates/pets/Cargo.toml",
        "[package]\nname = \"pets\"\nversion.workspace = \
         true\nlicense.workspace = true\nrepository.workspace = \
         true\ndescription = \"Pet care in Rust\"\n",
    );
    write(
        root,
        "README.md",
        "# Pets\n\nRead the [guide](docs/guide.md).\n",
    );
    write(root, "docs/guide.md", "# Guide\n\nFeed them.\n");
    write(root, ".gitignore", "generated/\n");
    write(root, "generated/broken.ttl", "not RDF");
    write(root, "CLAUDE.md", "# Agent instructions\n");
    write(root, "PLAN.md", "# Plan\n");
    write(root, "RELEASE_STATUS.md", "# Status\n");
    write(root, "tests/fixtures/broken.ttl", "not RDF");
    write(
        root,
        "crates/pets/tests/data/case.ttl",
        "@prefix : <x> . broken",
    );
}

async fn build(root: &Path, extra: &[&str]) -> Result<std::process::Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(root)
        .arg("--output")
        .arg(root.join("output/grapharch"))
        .arg("--offline")
        .args(extra)
        .output()
        .await?)
}

#[tokio::test]
async fn archive_copies_build_with_manifest_identity_and_no_meta_files()
-> Result<()> {
    let root = tempfile::tempdir()?;
    archive_copy(root.path());
    let result = build(root.path(), &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let site = root.path().join("output/grapharch/site");
    assert!(site.join("index.html").exists());
    assert!(site.join("docs/guide/index.html").exists());
    for excluded in ["CLAUDE", "PLAN", "RELEASE_STATUS"] {
        assert!(
            !site.join(format!("{excluded}/index.html")).exists(),
            "{excluded}"
        );
    }
    let graph = std::fs::read_to_string(
        root.path().join("output/grapharch/documentation.nq"),
    )?;
    assert!(graph.contains("<http://usefulinc.com/ns/doap#Project>"));
    assert!(
        graph.contains("\"3.1.4\""),
        "workspace version recorded"
    );
    assert!(graph.contains("<https://crates.io/crates/pets>"));
    assert!(graph.contains("<http://spdx.org/licenses/MIT>"));
    assert!(graph.contains("repository-forge-offline"));
    assert!(!graph.contains("Agent instructions"));
    let home = std::fs::read_to_string(site.join("index.html"))?;
    let document = scraper::Html::parse_document(&home);
    let chip = document
        .select(&scraper::Selector::parse(".brand-version").unwrap())
        .next()
        .expect("version chip");
    assert!(
        chip.text().collect::<String>().contains("v3.1.4"),
        "package version chip"
    );
    let facts = document
        .select(&scraper::Selector::parse(".project-facts").unwrap())
        .next()
        .expect("project facts")
        .html();
    for expected in [
        "https://github.com/example/pets",
        "https://github.com/example/pets/issues",
        "https://crates.io/crates/pets",
        "https://docs.rs/pets",
        "MIT",
    ] {
        assert!(facts.contains(expected), "{expected} in {facts}");
    }
    assert!(
        home.contains("<title>Home · pets</title>") ||
            home.contains("· pets</title>")
    );
    Ok(())
}

#[tokio::test]
async fn the_source_budget_is_configurable_and_named_in_its_error() -> Result<()>
{
    let root = tempfile::tempdir()?;
    archive_copy(root.path());
    let result = build(root.path(), &["--max-sources", "1"]).await?;
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("--max-sources"), "{stderr}");
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
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

const LINKS: &str = "# Guide\n\nSee [the manifest](../package.json), \
    [a missing page](/missing/) and the \
    [![build](https://img.shields.io/badge/build-passing-green?logo=github)](https://github.com/example/tool/issues/1).\n";

#[tokio::test]
async fn repository_files_link_to_the_forge_and_badges_are_never_fetched()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(
        r,
        "package.json",
        r#"{"name": "tool", "version": "1.4.0"}"#,
    );
    write(
        r,
        "README.md",
        "# Tool\n\nRead the [guide](docs/guide.md).\n",
    );
    write(r, "docs/guide.md", LINKS);
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "A"]);
    git(r, &["config", "user.email", "a@example.org"]);
    git(r, &[
        "remote",
        "add",
        "origin",
        "git@github.com:example/tool.git",
    ]);
    git(r, &["add", "."]);
    git(r, &["commit", "-q", "-m", "one"]);
    let commit = git(r, &["rev-parse", "HEAD"]);
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html = std::fs::read_to_string(
        r.join("output/grapharch/site/docs/guide/index.html"),
    )?;
    assert!(
        html.contains(&format!(
            "https://github.com/example/tool/blob/{commit}/package.json"
        )),
        "repository file links to the forge blob at the commit"
    );
    assert!(html.contains("a missing page") && !html.contains("/missing/"));
    assert!(html.contains(
        "https://img.shields.io/badge/build-passing-green?logo=github"
    ));
    let graph =
        std::fs::read_to_string(r.join("output/grapharch/documentation.nq"))?;
    for kind in [
        "RepositoryFileLink",
        "BrokenLink",
        "ForgeObjectLink",
        "PageLink",
    ] {
        assert!(
            graph.contains(&format!("grapharch#{kind}>")),
            "{kind} recorded"
        );
    }
    let strict = build(r, &["--strict"]).await?;
    assert!(!strict.status.success());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("/missing/"));
    Ok(())
}

#[tokio::test]
async fn archive_copies_link_repository_files_from_the_manifest_repository()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(
        r,
        "package.json",
        r#"{"name": "tool", "repository": "github:example/tool"}"#,
    );
    write(
        r,
        "README.md",
        "# Tool\n\nSee [the manifest](package.json).\n",
    );
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html =
        std::fs::read_to_string(r.join("output/grapharch/site/index.html"))?;
    assert!(
        html.contains("https://github.com/example/tool/blob/HEAD/package.json")
    );
    Ok(())
}
