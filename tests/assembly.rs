use {
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};

#[tokio::test]
async fn validated_revision_is_repeatable_and_other_projects_are_isolated()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let project = "urn:example:ontology-project";
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        project,
        "urn:collector:definitions",
        "tests/fixtures/publication/projects.ttl".into(),
    )?
    .collect(&writer)
    .await?;
    let assembler = Assembler::new(dataset.clone());
    let first = assembler.assemble(project).await?;
    assert_eq!(first, assembler.assemble(project).await?);
    let dir = tempfile::tempdir()?;
    let bad = dir.path().join("unrelated.ttl");
    tokio::fs::write(
        &bad,
        "<urn:broken> a <https://ekgf.org/ontology/grapharch#Target> .",
    )
    .await?;
    RdfFile::new("urn:unrelated", "urn:collector:bad", bad)?
        .collect(&writer)
        .await?;
    assert_eq!(first, assembler.assemble(project).await?);
    assert_eq!(Some(first), assembler.current(project).await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn invalid_update_preserves_last_valid_revision_and_historical_content()
-> anyhow::Result<()> {
    let dataset = Dataset::memory()?;
    let project = "urn:example:ontology-project";
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("project.ttl");
    let source = include_str!("fixtures/publication/projects.ttl");
    tokio::fs::write(&path, source).await?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let collector =
        RdfFile::new(project, "urn:collector:definitions", path.clone())?;
    collector.collect(&writer).await?;
    let assembler = Assembler::new(dataset.clone());
    let valid = assembler.assemble(project).await?;
    tokio::fs::write(
        &path,
        source.replace(
            "ex:spec a ga:Target ; ga:scope ex:family ;",
            "ex:spec a ga:Target ;",
        ),
    )
    .await?;
    collector.collect(&writer).await?;
    assert!(assembler.assemble(project).await.is_err());
    assert_eq!(
        Some(valid.clone()),
        assembler.current(project).await?
    );
    assert!(assembler.require_validated(project, &valid).await.is_ok());
    assert!(
        assembler
            .require_validated("urn:another", &valid)
            .await
            .is_err()
    );
    assert!(
        assembler
            .require_validated(project, &NamedNode::new("urn:missing")?)
            .await
            .is_err()
    );
    drop(writer);
    worker.await??;
    Ok(())
}
