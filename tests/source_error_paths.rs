//! Every failure while collecting an authored file names that file.
use grapharch::publication::{
    collect::{Collector, MarkdownFile},
    dataset::Dataset,
    writer::GraphWriter,
};

async fn collection_error(name: &str, body: &str) -> anyhow::Result<String> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(name);
    tokio::fs::write(&path, body).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset, 2, 1024 * 1024)?;
    let collector =
        MarkdownFile::new("urn:project", "urn:collector", path.clone())?;
    let error = collector
        .collect(&writer)
        .await
        .expect_err("collection must fail");
    drop(writer);
    worker.await??;
    let message = format!("{error:#}");
    assert!(
        message.contains(path.to_str().unwrap()),
        "error does not name {}: {message}",
        path.display()
    );
    Ok(message)
}

#[tokio::test]
async fn large_readme_with_many_code_spans_and_no_object_collects()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("README.md");
    let body = "Run `make` with `FLAG=1` in `dir/`.\n".repeat(1300);
    tokio::fs::write(&path, &body).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset, 2, 1024 * 1024)?;
    MarkdownFile::new("urn:project", "urn:collector", path)?
        .collect(&writer)
        .await?;
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn unsafe_svg_object_error_names_the_file() -> anyhow::Result<()> {
    let message = collection_error(
        "diagram.md",
        "<object data=\"a.svg\" type=\"image/svg+xml\" \
         onload=\"x()\"></object>\n",
    )
    .await?;
    assert!(message.contains("SVG"), "{message}");
    Ok(())
}

#[tokio::test]
async fn invalid_frontmatter_error_names_the_file() -> anyhow::Result<()> {
    collection_error(
        "broken.md",
        "---\ngrapharch: [broken\n---\nBody.\n",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn unterminated_container_error_names_the_file() -> anyhow::Result<()> {
    collection_error("note.md", "---\ntitle: x\nBody without end.\n").await?;
    Ok(())
}
