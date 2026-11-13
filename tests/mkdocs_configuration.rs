use {
    anyhow::Result,
    grapharch::publication::{
        quick_build::BuildRequest,
        render::{PublishOutcome, Publisher},
    },
};

#[tokio::test]
async fn appended_dictionary_resolves_under_snippet_roots_without_becoming_a_page()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("mkdocs.yml"),
        "site_name: Knowledge\nmarkdown_extensions:\n  - abbr\n  - \
         pymdownx.snippets:\n      base_path: [docs]\n      auto_append: \
         [abbreviations.md]\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "# Knowledge\n\nAn EKG connects meaning.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/abbreviations.md"),
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
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&plan, output.path())
        .await?
    else {
        panic!("superseded")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(
        html.contains("<abbr title=\"Enterprise Knowledge Graph\">EKG</abbr>")
    );
    assert!(
        !publication
            .directory
            .join("abbreviations/index.html")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn native_pages_hints_reject_yaml_aliases() -> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("mkdocs.yml"),
        "site_name: Knowledge\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "# Knowledge\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/.pages"),
        "nav: &menu\n  - index.md\n",
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
        "navigation YAML must not expand aliases before checking limits"
    );
    Ok(())
}
