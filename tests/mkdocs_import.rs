use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        quick_build::BuildRequest,
        render::{PublishOutcome, Publisher},
    },
};

#[tokio::test]
async fn mkdocs_nested_navigation_survives_source_removal() -> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("manual/guide")).await?;
    tokio::fs::write(source.path().join("mkdocs.yml"), "site_name: Orchard handbook\nsite_description: Growing fruit\ndocs_dir: manual\ntheme: material\nplugins: [search]\nmarkdown_extensions: [admonition, tables, footnotes]\nnav:\n  - Welcome: index.md\n  - Growing:\n      - Pruning: guide/pruning.md\n      - Revisit: guide/pruning.md\n  - Reference: https://example.org/reference\n").await?;
    tokio::fs::write(
        source.path().join("manual/index.md"),
        "# Orchard\n\nRead [pruning](guide/pruning.md).\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("manual/guide/pruning.md"),
        "# Pruning\n\n!!! note\n    Keep healthy branches.\n",
    )
    .await?;
    let (dataset, plan) = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().to_owned(),
    }
    .collect()
    .await?;
    let archive = output.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("superseded")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("Orchard handbook"));
    assert!(html.contains("Growing"));
    assert!(html.contains("Revisit"));
    assert!(html.contains("https://example.org/reference"));
    assert!(!html.contains("manual/guide/pruning/"));
    assert!(
        publication
            .directory
            .join("guide/pruning/index.html")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn mkdocs_rejects_unsupported_execution_and_invalid_paths() -> Result<()>
{
    for (extra, expected) in [
        ("hooks: [hooks.py]\n", "hooks"),
        ("plugins: [mkdocstrings]\n", "mkdocstrings"),
        (
            "theme: !!python/object/apply:os.system ['touch BAD']\n",
            "YAML tags",
        ),
        (
            "docs_dir: ../outside\n",
            "relative without traversal",
        ),
        (
            "markdown_extensions:\n  - pymdownx.superfences:\n      \
             custom_fences: [{name: unknown}]\n",
            "custom_fences",
        ),
        (
            "plugins:\n  - search:\n      lang: [ja]\n",
            "lang",
        ),
    ] {
        let source = tempfile::tempdir()?;
        tokio::fs::create_dir(source.path().join("docs")).await?;
        tokio::fs::write(source.path().join("docs/index.md"), "# Home\n")
            .await?;
        tokio::fs::write(
            source.path().join("mkdocs.yml"),
            format!("site_name: Fixture\n{extra}"),
        )
        .await?;
        let result = BuildRequest {
            source: source.path().to_owned(),
            output: source.path().join("output"),
        }
        .collect()
        .await;
        let error = result.err().expect("unsupported settings must fail");
        assert!(
            format!("{error:#}").contains(expected),
            "{extra}: {error:#}"
        );
        assert!(!source.path().join("BAD").exists());
    }
    Ok(())
}

#[tokio::test]
async fn mkdocs_native_metadata_and_default_directory_navigation() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("docs/guide")).await?;
    tokio::fs::write(
        source.path().join("mkdocs.yaml"),
        "site_name: Orchard\ntheme: mkdocs\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "title: Welcome home\n\n# Orchard\n\nStart growing.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/guide/pruning.md"),
        "title: Keeping trees healthy\n\n# Pruning\n\nCare.\n",
    )
    .await?;
    let (dataset, plan) = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().to_owned(),
    }
    .collect()
    .await?;
    let PublishOutcome::Published(publication) =
        Publisher::new(dataset).render(&plan, output.path()).await?
    else {
        panic!("superseded")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("Welcome home"));
    assert!(html.contains("Keeping trees healthy"));
    assert!(!html.contains("title: Welcome"));
    Ok(())
}
