use {
    crate::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::Result,
};

#[tokio::test]
#[ignore = "acceptance against the sibling ekg-maturity source checkout"]
async fn real_native_maturity_pages_retain_authoritative_companions()
-> Result<()> {
    let root = tokio::fs::canonicalize("../ekg-maturity").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    RdfFile::new(
        "urn:ekg-maturity",
        "urn:definitions",
        root.join(".grapharch/project.ttl"),
    )?
    .collect(&writer)
    .await?;
    let mut entries = tokio::fs::read_dir(root.join("metadata")).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "ttl")
        {
            RdfFile::new("urn:ekg-maturity", "urn:metadata", entry.path())?
                .collect(&writer)
                .await?;
        }
    }
    super::collect(&root, &dataset, &writer, "urn:ekg-maturity", 2).await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:ekg-maturity").await?,
    );
    let mut pages = dataset
        .select(format!(
            "SELECT DISTINCT ?page WHERE {{ {} }}",
            view.pattern(&format!("?page a {}", term("Page")), 0)
        ))
        .await?;
    let mut count = 0;
    while pages.next().await?.is_some() {
        count += 1;
    }
    assert_eq!(count, 92);
    let archive = std::path::Path::new("output/maturity-native-acceptance.nq");
    tokio::fs::create_dir_all("output").await?;
    dataset.export(archive.to_path_buf()).await?;
    let text = tokio::fs::read_to_string(archive).await?;
    assert!(text.contains("Shared goals") || text.contains("Business terms"));
    assert!(text.contains("docs-fragments/pillar/"));
    drop(writer);
    worker.await??;
    Ok(())
}
