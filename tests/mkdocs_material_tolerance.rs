//! The four public EKGF Material for MkDocs configurations import unchanged:
//! Python YAML tags are read as data, plugins map to GraphArch features or
//! warnings, and nothing named by the configuration is executed.
use {
    anyhow::Result,
    grapharch::publication::quick_build::BuildRequest,
    std::path::Path,
};

async fn collects(configuration: &str) -> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let root = source.path();
    tokio::fs::write(root.join("mkdocs.yml"), configuration).await?;
    for directory in ["docs", "docs/diagrams/src", "docs/stylesheets"] {
        tokio::fs::create_dir_all(root.join(directory)).await?;
    }
    tokio::fs::write(root.join("docs/index.md"), "# Home\n\nAn EKG.\n").await?;
    tokio::fs::write(
        root.join("abbreviations.md"),
        "*[EKG]: Knowledge\n",
    )
    .await?;
    // A hook named by the configuration must never run.
    tokio::fs::write(
        root.join("docs/main.py"),
        "open('executed', 'w').close()\n",
    )
    .await?;
    BuildRequest {
        source:  root.to_owned(),
        output:  output.path().to_owned(),
        options: grapharch::publication::quick_build::BuildOptions::offline(),
    }
    .collect()
    .await?;
    assert!(!Path::new(&root.join("executed")).exists());
    Ok(())
}

#[tokio::test]
async fn ekg_principles_configuration_imports_unchanged() -> Result<()> {
    collects(include_str!(
        "fixtures/mkdocs-material/ekg-principles.yml"
    ))
    .await
}

#[tokio::test]
async fn ekg_method_configuration_imports_unchanged() -> Result<()> {
    collects(include_str!(
        "fixtures/mkdocs-material/ekg-method.yml"
    ))
    .await
}

#[tokio::test]
async fn ekg_maturity_configuration_imports_unchanged() -> Result<()> {
    collects(include_str!(
        "fixtures/mkdocs-material/ekg-maturity.yml"
    ))
    .await
}

#[tokio::test]
async fn ekg_catalog_configuration_imports_unchanged() -> Result<()> {
    collects(include_str!(
        "fixtures/mkdocs-material/ekg-catalog.yml"
    ))
    .await
}

#[tokio::test]
async fn unknown_python_tag_is_a_diagnostic_not_a_failure() -> Result<()> {
    collects(
        "site_name: Knowledge\nmarkdown_extensions:\n  - pymdownx.emoji:\n      \
         emoji_index: !!python/name:os.system\n",
    )
    .await
}

#[tokio::test]
async fn hooks_and_unknown_plugins_are_reported_but_never_run() -> Result<()> {
    collects(
        "site_name: Knowledge\nhooks: [docs/main.py]\nplugins:\n  - \
         mkdocstrings\n  - search:\n      lang: [ja]\n",
    )
    .await
}
