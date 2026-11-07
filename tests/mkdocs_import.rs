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
    tokio::fs::write(source.path().join("mkdocs.yml"), "site_name: Orchard handbook\nsite_description: Growing fruit\nsite_url: https://example.org/orchard/\ndocs_dir: manual\ntheme: material\nplugins: [search]\nmarkdown_extensions: [admonition, tables, footnotes]\nnav:\n  - Welcome: index.md\n  - Growing:\n      - Pruning: guide/pruning.md\n      - Revisit: guide/pruning.md\n  - Reference: https://example.org/reference\n").await?;
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
    let retained = tokio::fs::read_to_string(&archive).await?;
    assert!(retained.contains("site_name: Orchard handbook\\n"));
    assert!(retained.contains("mkdocs-presentation-substitution"));
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
    assert!(html.contains("name=\"description\" content=\"Growing fruit\""));
    assert!(
        html.contains(
            "rel=\"canonical\" href=\"https://example.org/orchard/\""
        )
    );
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
        ("theme: [material]\n", "theme must"),
        ("use_directory_urls: bananas\n", "must be true"),
        ("site_url: ftp://example.org/\n", "HTTP(S)"),
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
        "site_name: Orchard\nsite_author: Alice & Bob\ntheme: mkdocs\n",
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

#[tokio::test]
async fn mkdocs_snippets_and_abbreviation_dictionary_use_explicit_roots()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("mkdocs.yml"),
        "site_name: Knowledge\nmarkdown_extensions:\n  - abbr\n  - \
         pymdownx.snippets:\n      base_path: ['.', docs]\n      check_paths: \
         true\n      auto_append: [abbreviations.md]\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "# Knowledge\n\n--8<-- \"shared.md\"\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("shared.md"),
        "An EKG connects meaning.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("abbreviations.md"),
        "*[EKG]: Enterprise Knowledge Graph\n",
    )
    .await?;
    let (dataset, plan) = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().to_owned(),
    }
    .collect()
    .await?;
    source.close()?;
    let PublishOutcome::Published(publication) =
        Publisher::new(dataset).render(&plan, output.path()).await?
    else {
        panic!("superseded")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(
        html.contains("<abbr title=\"Enterprise Knowledge Graph\">EKG</abbr>")
    );
    assert!(html.contains("connects meaning"));
    Ok(())
}

#[tokio::test]
async fn explicit_grapharch_configuration_precedes_mkdocs_even_when_invalid()
-> Result<()> {
    for name in ["grapharch.ttl", "project.ttl", ".grapharch/project.ttl"] {
        let source = tempfile::tempdir()?;
        tokio::fs::create_dir_all(source.path().join("docs")).await?;
        tokio::fs::create_dir_all(source.path().join(".grapharch")).await?;
        tokio::fs::write(
            source.path().join("docs/index.md"),
            "# Valid Markdown\n",
        )
        .await?;
        tokio::fs::write(
            source.path().join("mkdocs.yml"),
            "site_name: MkDocs must not win\n",
        )
        .await?;
        tokio::fs::write(
            source.path().join(name),
            "@prefix this is not Turtle",
        )
        .await?;
        let result = BuildRequest {
            source: source.path().to_owned(),
            output: source.path().join("output"),
        }
        .collect()
        .await;
        assert!(
            result.is_err(),
            "invalid explicit {name} must not fall back to MkDocs"
        );
    }
    Ok(())
}

#[tokio::test]
async fn mkdocs_navigation_anchors_and_source_boundary_are_checked()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "# Home\n\n## Fruit\n\nTrees.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("broken.ttl"),
        "not RDF and outside docs_dir",
    )
    .await?;
    let config = "site_name: Orchard\nnav:\n  - Fruit: index.md#fruit\n  - \
                  Mail: mailto:team@example.org\n";
    tokio::fs::write(source.path().join("mkdocs.yml"), config).await?;
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
    assert!(
        html.contains("href=\"./#fruit\"") || html.contains("href=\"#fruit\"")
    );
    tokio::fs::write(source.path().join("mkdocs.yaml"), config).await?;
    let error = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().to_owned(),
    }
    .collect()
    .await
    .err()
    .expect("ambiguous config must fail");
    assert!(
        error
            .to_string()
            .contains("both mkdocs.yml and mkdocs.yaml")
    );
    Ok(())
}
