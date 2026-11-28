use {
    super::Manifests,
    crate::publication::repository::package::Ecosystem,
    std::path::Path,
};

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[tokio::test]
async fn cargo_workspaces_inherit_package_fields() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
exclude = ["crates/skip"]
[workspace.package]
version = "0.4.1"
license = "MIT OR Apache-2.0"
repository = "https://github.com/o/r"
keywords = ["rdf", "graph"]
"#,
    );
    write(
        root.path(),
        "crates/core/Cargo.toml",
        r#"
[package]
name = "r-core"
version.workspace = true
license.workspace = true
repository.workspace = true
keywords.workspace = true
description = "Core types"
categories = ["parsing"]
"#,
    );
    write(
        root.path(),
        "crates/cli/Cargo.toml",
        r#"
[package]
name = "r-cli"
version.workspace = true
publish = false
"#,
    );
    write(
        root.path(),
        "crates/skip/Cargo.toml",
        "[package]\nname = \"skip\"\n",
    );
    let manifests = Manifests::read(root.path()).await.unwrap();
    let names: Vec<_> = manifests
        .packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, ["r-cli", "r-core"]);
    let core = &manifests.packages()[1];
    assert_eq!(core.version.as_ref().unwrap().as_str(), "0.4.1");
    assert_eq!(
        core.license.as_ref().unwrap().as_str(),
        "MIT OR Apache-2.0"
    );
    assert_eq!(core.keywords, ["rdf", "graph"]);
    assert_eq!(core.categories, ["parsing"]);
    assert_eq!(
        core.registry_page().unwrap().as_str(),
        "https://crates.io/crates/r-core"
    );
    assert_eq!(
        core.api_reference().unwrap().as_str(),
        "https://docs.rs/r-core"
    );
    assert!(manifests.packages()[0].registry_page().is_none());
    assert_eq!(
        manifests.common(|p| p.version.as_ref().map(|v| v.as_str().to_owned())),
        Some("0.4.1".to_owned())
    );
}

#[tokio::test]
async fn pyproject_reads_pep_621_metadata() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "pyproject.toml",
        r#"
[project]
name = "ekglib"
version = "1.2.3.post1"
description = "EKG library"
license = { text = "MIT" }
keywords = ["ekg"]
classifiers = ["Topic :: Software Development"]
[project.urls]
Homepage = "https://ekgf.org"
Repository = "https://github.com/EKGF/ekg-lib"
"#,
    );
    let manifests = Manifests::read(root.path()).await.unwrap();
    let package = manifests.primary().unwrap();
    assert_eq!(package.ecosystem, Ecosystem::PyPI);
    assert_eq!(
        package.version.as_ref().unwrap().as_str(),
        "1.2.3.post1"
    );
    assert!(package.version.as_ref().unwrap().semver().is_none());
    assert_eq!(
        package
            .license
            .as_ref()
            .unwrap()
            .license_iri()
            .unwrap()
            .as_str(),
        "http://spdx.org/licenses/MIT"
    );
    assert_eq!(
        package.repository.as_ref().unwrap().as_str(),
        "https://github.com/EKGF/ekg-lib"
    );
    assert_eq!(package.categories, ["Software Development"]);
    assert_eq!(
        package.registry_page().unwrap().as_str(),
        "https://pypi.org/project/ekglib/"
    );
}

#[tokio::test]
async fn package_json_reads_workspaces_and_repository_forms() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "package.json",
        r#"{
  "name": "@scope/tool", "version": "2.0.0-beta.1", "license": "Apache-2.0",
  "description": "A tool", "repository": {"type": "git",
  "url": "git+https://github.com/o/tool.git"}, "workspaces": ["packages/*"]
}"#,
    );
    write(
        root.path(),
        "packages/a/package.json",
        r#"{"name": "a", "private": true, "repository": "github:o/tool"}"#,
    );
    let manifests = Manifests::read(root.path()).await.unwrap();
    let primary = manifests.primary().unwrap();
    assert_eq!(primary.name.as_str(), "@scope/tool");
    assert_eq!(
        primary
            .version
            .as_ref()
            .unwrap()
            .semver()
            .unwrap()
            .pre
            .as_str(),
        "beta.1"
    );
    assert_eq!(
        primary.repository.as_ref().unwrap().as_str(),
        "https://github.com/o/tool.git"
    );
    assert_eq!(
        primary.registry_page().unwrap().as_str(),
        "https://www.npmjs.com/package/@scope/tool"
    );
    let member = &manifests.packages()[1];
    assert!(member.registry_page().is_none());
    assert_eq!(
        member.repository.as_ref().unwrap().as_str(),
        "https://github.com/o/tool"
    );
}

#[tokio::test]
async fn go_modules_name_their_module_path() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "go.mod",
        "module github.com/o/svc\n\ngo 1.22\n",
    );
    let manifests = Manifests::read(root.path()).await.unwrap();
    let package = manifests.primary().unwrap();
    assert_eq!(package.name.as_str(), "github.com/o/svc");
    assert_eq!(
        package.registry_page().unwrap().as_str(),
        "https://pkg.go.dev/github.com/o/svc"
    );
}

#[tokio::test]
async fn malformed_manifests_are_errors_and_absent_ones_are_empty() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        Manifests::read(root.path())
            .await
            .unwrap()
            .packages()
            .is_empty()
    );
    write(root.path(), "Cargo.toml", "[package\nname=");
    assert!(Manifests::read(root.path()).await.is_err());
}
