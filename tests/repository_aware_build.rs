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
    [a deleted file](../gone.json), \
    [a missing page](/missing/) and the \
    [![build](https://img.shields.io/badge/build-passing-green?logo=github)](https://github.com/example/tool/issues/1).\n\n\
    ```plantuml\n@startuml\n!include common.puml\n@enduml\n```\n";

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
    assert!(
        html.contains("a deleted file") && !html.contains("gone.json"),
        "a link to a file absent from the Git index is broken, not a forge \
         link"
    );
    assert!(html.contains(
        "https://img.shields.io/badge/build-passing-green?logo=github"
    ));
    let graph =
        std::fs::read_to_string(r.join("output/grapharch/documentation.nq"))?;
    assert!(
        graph.contains("grapharch#DiagramIncludeContext>"),
        "diagram include recorded"
    );
    assert!(graph.contains("grapharch#repositoryFileExists>"));
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
    let stderr = String::from_utf8_lossy(&strict.stderr);
    assert!(
        stderr.contains("/missing/") || stderr.contains("gone.json"),
        "{stderr}"
    );
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

#[tokio::test]
async fn workspaces_without_one_version_list_their_crates() -> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(
        r,
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    );
    write(
        r,
        "a/Cargo.toml",
        "[package]\nname = \"kit-a\"\nversion = \"0.1.0\"\n",
    );
    write(
        r,
        "b/Cargo.toml",
        "[package]\nname = \"kit-b\"\nversion = \"0.2.0\"\n",
    );
    write(r, "README.md", "# Kit\n");
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let home =
        std::fs::read_to_string(r.join("output/grapharch/site/index.html"))?;
    let document = scraper::Html::parse_document(&home);
    let chip = document
        .select(&scraper::Selector::parse(".brand-version").unwrap())
        .next()
        .expect("version chip element");
    assert!(
        chip.value().attr("hidden").is_some(),
        "no single version: chip hidden"
    );
    let facts = document
        .select(&scraper::Selector::parse(".project-facts").unwrap())
        .next()
        .expect("project facts")
        .html();
    for expected in [
        "kit-a",
        "0.1.0",
        "kit-b",
        "0.2.0",
        "https://crates.io/crates/kit-b",
    ] {
        assert!(facts.contains(expected), "{expected} in {facts}");
    }
    Ok(())
}

#[tokio::test]
async fn mkdocs_navigation_wins_over_default_exclusions() -> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(
        r,
        "mkdocs.yml",
        "site_name: Process\nnav:\n  - Home: index.md\n  - Build: \
         process/build/index.md\n  - Tests: tests/strategy.md\n",
    );
    write(r, "docs/index.md", "# Home\n");
    write(
        r,
        "docs/process/build/index.md",
        "# Build process\n",
    );
    write(r, "docs/tests/strategy.md", "# Test strategy\n");
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let site = r.join("output/grapharch/site");
    let found = |needle: &str| {
        walk(&site).iter().any(|path| {
            std::fs::read_to_string(path)
                .is_ok_and(|html| html.contains(needle))
        })
    };
    assert!(
        found("Build process"),
        "docs/process/build is content"
    );
    assert!(
        found("Test strategy"),
        "a navigated tests/ page is collected"
    );
    Ok(())
}

fn walk(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "html") {
                found.push(path);
            }
        }
    }
    found
}

#[tokio::test]
async fn links_in_rdf_descriptions_are_recorded_references() -> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(r, "README.md", "# Pets\n");
    write(
        r,
        "pets.ttl",
        "@prefix owl: <http://www.w3.org/2002/07/owl#> .\n\
         @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         @prefix an: <https://ekgf.org/ontology/annotation#> .\n\
         <https://example.org/pets#Pet> a owl:Class ; rdfs:label \"Pet\" ;\n\
         an:markdownDescription \"See [care](https://example.org/care).\" .\n",
    );
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let graph =
        std::fs::read_to_string(r.join("output/grapharch/documentation.nq"))?;
    assert!(
        graph.contains("grapharch#DescriptionContext>"),
        "description link recorded"
    );
    assert!(graph.contains("\"https://example.org/care\""));
    Ok(())
}

#[tokio::test]
async fn pages_show_their_last_git_change() -> Result<()> {
    let root = tempfile::tempdir()?;
    let r = root.path();
    write(
        r,
        "README.md",
        "# Home\n\nSee the [guide](guide.md).\n",
    );
    write(r, "guide.md", "# Guide\n");
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "Ada"]);
    git(r, &["config", "user.email", "a@example.org"]);
    git(r, &["add", "."]);
    let first = "2024-01-02T03:04:05Z";
    let second = "2025-06-07T08:09:10Z";
    let commit = |date: &str, message: &str| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(r)
            .args(["commit", "-q", "-m", message])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap();
        assert!(output.status.success());
    };
    commit(first, "one");
    write(r, "guide.md", "# Guide\n\nMore.\n");
    git(r, &["add", "guide.md"]);
    commit(second, "two");
    let result = build(r, &[]).await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let site = r.join("output/grapharch/site");
    let home = std::fs::read_to_string(site.join("index.html"))?;
    let guide = std::fs::read_to_string(site.join("guide/index.html"))?;
    assert!(
        home.contains("title=\"Last update: 2 January 2024\""),
        "{home}"
    );
    assert!(home.contains("<time datetime=\"2024-01-02"));
    assert!(guide.contains("<time datetime=\"2025-06-07"));
    assert!(guide.contains(">7 June 2025</time>"));
    // The guide was created by the first commit.
    assert!(guide.contains("title=\"Created: 2 January 2024\""));
    let graph =
        std::fs::read_to_string(r.join("output/grapharch/documentation.nq"))?;
    assert!(graph.contains("<http://purl.org/dc/terms/modified>"));

    // Without .git, no date is shown.
    std::fs::remove_dir_all(r.join(".git"))?;
    std::fs::remove_dir_all(r.join("output"))?;
    let result = build(r, &[]).await?;
    assert!(result.status.success());
    let home = std::fs::read_to_string(site.join("index.html"))?;
    assert!(!home.contains("page-dates"));
    Ok(())
}
