use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        quick_build::BuildRequest,
        render::{PublishOutcome, Publisher},
    },
};

const BOOK: &str = "[book]\ntitle = \"The Orchard Book\"\ndescription = \
                    \"Growing fruit\"\nsrc = \
                    \"chapters\"\n\n[preprocessor.katex]\ncommand = \"touch \
                    BAD\"\n\n[output.html]\nadditional-js = \
                    [\"custom.js\"]\n\n[output.linkcheck]\n";

const SUMMARY: &str = "# Summary\n\n[Preface](./preface.md)\n\n# Growing\n\n- \
                       [Planting](./planting.md)\n  - \
                       [Pruning](./guide/pruning.md)\n- \
                       [Draft]()\n\n---\n\n[Appendix](./appendix.md)\n";

async fn book() -> Result<tempfile::TempDir> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("chapters/guide")).await?;
    tokio::fs::write(source.path().join("book.toml"), BOOK).await?;
    for (path, body) in [
        ("chapters/SUMMARY.md", SUMMARY),
        ("chapters/preface.md", "# Preface\n\nWelcome.\n"),
        ("chapters/planting.md", "# Planting\n\nDig.\n"),
        ("chapters/guide/pruning.md", "# Pruning\n\nCut.\n"),
        ("chapters/appendix.md", "# Appendix\n\nMore.\n"),
    ] {
        tokio::fs::write(source.path().join(path), body).await?;
    }
    Ok(source)
}

#[tokio::test]
async fn mdbook_summary_becomes_navigation_without_executing_anything()
-> Result<()> {
    let source = book().await?;
    let output = tempfile::tempdir()?;
    let (dataset, plan) = BuildRequest {
        source: source.path().to_owned(),
        output: output.path().to_owned(),
    }
    .collect()
    .await?;
    let archive = output.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    let retained = tokio::fs::read_to_string(&archive).await?;
    assert!(retained.contains("mdbook-preprocessor-not-executed"));
    assert!(retained.contains("mdbook-output-not-produced"));
    assert!(retained.contains("mdbook-presentation-substitution"));
    let root = source.path().to_owned();
    source.close()?;
    assert!(!root.join("BAD").exists());
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&plan, output.path())
        .await?
    else {
        panic!("superseded")
    };
    let page = publication.directory.join("planting/index.html");
    let html = tokio::fs::read_to_string(&page).await?;
    assert!(html.contains("The Orchard Book"));
    for label in ["Preface", "Growing", "Pruning", "Appendix"] {
        assert!(html.contains(label), "navigation lacks {label}");
    }
    assert!(
        !html.contains(">Draft<"),
        "draft chapters have no page"
    );
    let home =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(
        home.contains("Welcome."),
        "the first chapter is the home page"
    );
    assert!(publication.directory.join("preface/index.html").exists());
    assert!(
        publication
            .directory
            .join("guide/pruning/index.html")
            .exists()
    );
    assert!(!publication.directory.join("SUMMARY/index.html").exists());
    Ok(())
}

#[tokio::test]
async fn mdbook_rejects_unsafe_summary_destinations() -> Result<()> {
    for (summary, expected) in [
        (
            "- [Out](../outside.md)\n",
            "relative to the book source",
        ),
        ("- [Missing](./missing.md)\n", "missing.md"),
        (
            "- [Script](javascript:alert(1))\n",
            "relative to the book source",
        ),
    ] {
        let source = book().await?;
        tokio::fs::write(source.path().join("chapters/SUMMARY.md"), summary)
            .await?;
        let error = BuildRequest {
            source: source.path().to_owned(),
            output: source.path().join("output"),
        }
        .collect()
        .await
        .err()
        .map(|error| format!("{error:#}"))
        .unwrap_or_default();
        assert!(error.contains(expected), "{summary}: {error}");
    }
    Ok(())
}
