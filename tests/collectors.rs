use {
    grapharch::publication::{
        collect::{Collector, MarkdownFile, RdfFile, SparqlFile},
        dataset::Dataset,
        writer::GraphWriter,
    },
    oxigraph::model::{GraphName, Literal, NamedNode, Quad},
};

#[tokio::test]
async fn markdown_frontmatter_is_normalized_and_repeated_collection_is_unchanged()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("intro.md");
    tokio::fs::write(
        &path,
        "---\ntitle: Introduction\ngrapharch:\n  subject: \
         urn:domain:ontology\n---\n# Introduction\n\nShared prose.\n",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let collector =
        MarkdownFile::new("urn:project:test", "urn:collector:markdown", path)?;
    collector.collect(&writer).await?;
    let before = dataset.snapshot().await?;
    collector.collect(&writer).await?;
    assert_eq!(before, dataset.snapshot().await?);
    assert!(
        dataset
            .contains(
                &before,
                Quad::new(
                    collector.fragment_id(),
                    NamedNode::new(
                        "https://ekgf.org/ontology/grapharch#subject"
                    )?,
                    NamedNode::new("urn:domain:ontology")?,
                    GraphName::DefaultGraph
                )
            )
            .await?
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn malformed_rdf_does_not_publish_a_partial_contribution()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ontology.ttl");
    tokio::fs::write(&path, "<urn:a> <urn:b> <urn:c> .\n<broken").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    assert!(
        RdfFile::new("urn:project:test", "urn:collector:rdf", path)?
            .collect(&writer)
            .await
            .is_err()
    );
    assert_eq!(dataset.active_count().await?, 0);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn sparql_is_documented_and_never_executed() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("danger.rq");
    let query = "# grapharch:subject urn:domain:example\nDROP ALL";
    tokio::fs::write(&path, query).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let collector =
        SparqlFile::new("urn:project:test", "urn:collector:sparql", path)?;
    collector.collect(&writer).await?;
    let snapshot = dataset.snapshot().await?;
    assert!(
        dataset
            .contains(
                &snapshot,
                Quad::new(
                    collector.fragment_id(),
                    NamedNode::new("https://ekgf.org/ontology/grapharch#body")?,
                    Literal::new_simple_literal(query),
                    GraphName::DefaultGraph
                )
            )
            .await?
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn anonymous_rdf_nodes_are_stable_across_recollection()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("anonymous.ttl");
    tokio::fs::write(
        &path,
        "<urn:a> <urn:b> [ <urn:label> \"anonymous\" ] .",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let collector =
        RdfFile::new("urn:project:test", "urn:collector:rdf", path)?;
    collector.collect(&writer).await?;
    let first = dataset.snapshot().await?;
    collector.collect(&writer).await?;
    assert_eq!(first, dataset.snapshot().await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn equivalent_frontmatter_and_sparql_hints_select_the_same_subject()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let md = dir.path().join("intro.md");
    let rq = dir.path().join("query.rq");
    tokio::fs::write(
        &md,
        "---\ngrapharch:\n  subject: urn:domain:shared\n---\nDescription.",
    )
    .await?;
    tokio::fs::write(
        &rq,
        "# grapharch:subject urn:domain:shared\nSELECT * WHERE { ?s ?p ?o }",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let markdown =
        MarkdownFile::new("urn:project:test", "urn:collector:md", md)?;
    let sparql = SparqlFile::new("urn:project:test", "urn:collector:rq", rq)?;
    markdown.collect(&writer).await?;
    sparql.collect(&writer).await?;
    let snapshot = dataset.snapshot().await?;
    for fragment in [markdown.fragment_id(), sparql.fragment_id()] {
        assert!(
            dataset
                .contains(
                    &snapshot,
                    Quad::new(
                        fragment,
                        NamedNode::new(
                            "https://ekgf.org/ontology/grapharch#subject"
                        )?,
                        NamedNode::new("urn:domain:shared")?,
                        GraphName::DefaultGraph
                    )
                )
                .await?
        );
    }
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn rdf_chunks_split_on_bytes_as_well_as_quad_count() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("large-literals.nt");
    let literal = "x".repeat(600 * 1024);
    tokio::fs::write(
        &path,
        format!(
            "<urn:a> <urn:p> \"{literal}\" .\n<urn:b> <urn:p> \"{literal}\" ."
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    RdfFile::new("urn:project", "urn:collector", path)?
        .collect(&writer)
        .await?;
    assert_eq!(dataset.active_count().await?, 2);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn invalid_edit_is_recorded_even_when_parsing_fails_before_any_output()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("intro.md");
    tokio::fs::write(&path, "Initial valid content.").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let collector =
        MarkdownFile::new("urn:project", "urn:collector", path.clone())?;
    collector.collect(&writer).await?;
    let before = dataset.snapshot().await?;
    tokio::fs::write(&path, "---\ngrapharch: [broken\n---\nContent.").await?;
    assert!(collector.collect(&writer).await.is_err());
    assert!(dataset.ensure_ready("urn:project").await.is_err());
    assert_eq!(before, dataset.snapshot().await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn blank_node_identity_is_isolated_by_project() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("blank.ttl");
    tokio::fs::write(&path, "[] <urn:predicate> \"value\" .").await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    for project in ["urn:project:a", "urn:project:b"] {
        RdfFile::new(project, "urn:collector:shared", path.clone())?
            .collect(&writer)
            .await?;
    }
    let snapshot = dataset.snapshot().await?;
    let pattern = Dataset::logical_pattern(
        &snapshot,
        "?subject <urn:predicate> \"value\"",
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut count = 0;
    while rows.next().await?.is_some() {
        count += 1;
    }
    assert_eq!(count, 2);
    drop(writer);
    worker.await??;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn rdf_base_iri_is_part_of_the_source_revision() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let first = dir.path().join("first.ttl");
    let second = dir.path().join("second.ttl");
    let link = dir.path().join("source.ttl");
    for path in [&first, &second] {
        tokio::fs::write(path, "<#subject> <urn:predicate> <#object> .")
            .await?;
    }
    tokio::fs::symlink(&first, &link).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
    let collector = RdfFile::new("urn:project", "urn:collector", link.clone())?;
    collector.collect(&writer).await?;
    let before = dataset.snapshot().await?;
    tokio::fs::remove_file(&link).await?;
    tokio::fs::symlink(&second, &link).await?;
    collector.collect(&writer).await?;
    assert_ne!(before, dataset.snapshot().await?);
    assert_eq!(dataset.active_count().await?, 1);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn keep_going_collectors_finish_independent_sources_and_report_failures()
-> anyhow::Result<()> {
    use {
        grapharch::publication::{
            collect::{MarkdownFile, collect_all_with_mode},
            diagnostics::FailureMode,
        },
        std::sync::Arc,
    };
    for mode in [FailureMode::FailFast, FailureMode::KeepGoing] {
        let temp = tempfile::tempdir()?;
        let good = temp.path().join("good.md");
        tokio::fs::write(&good, "Independent valid content.").await?;
        let dataset = Dataset::memory()?;
        let before = dataset.snapshot_for("urn:project").await?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        let collectors: Vec<Arc<dyn Collector>> = vec![
            Arc::new(MarkdownFile::new(
                "urn:project",
                "urn:missing",
                temp.path().join("missing.md"),
            )?),
            Arc::new(MarkdownFile::new(
                "urn:project",
                "urn:good",
                good,
            )?),
        ];
        let report = collect_all_with_mode(collectors, &writer, 1, mode).await;
        if mode == FailureMode::FailFast {
            assert!(report.is_err());
            assert_eq!(dataset.snapshot_for("urn:project").await?, before);
        } else {
            let report = report?;
            assert_eq!(report.completed, 1);
            assert_eq!(report.errors.len(), 1);
            assert_ne!(dataset.snapshot_for("urn:project").await?, before);
            let mut diagnostics = dataset.select("SELECT ?diagnostic WHERE { GRAPH ?g { ?diagnostic a <https://ekgf.org/ontology/grapharch#Diagnostic> } } LIMIT 1".into()).await?;
            assert!(diagnostics.next().await?.is_some());
        }
        assert!(dataset.ensure_ready("urn:project").await.is_err());
        drop(writer);
        worker.await??;
    }
    Ok(())
}
