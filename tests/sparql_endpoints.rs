#[path = "sparql_support/mod.rs"]
mod support;
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, SparqlEndpointSource, SparqlSourceConfig},
        dataset::Dataset,
        writer::GraphWriter,
    },
    oxrdf::{GraphName, NamedNode, Quad},
};
fn source(server: &support::Server) -> SparqlSourceConfig {
    serde_yaml::from_str(&format!(
        "endpoint: {}/source\nquery: CONSTRUCT {{ ?s ?p ?o }} WHERE {{ ?s ?p \
         ?o }}\nmax_source_bytes: 65536\n",
        server.base
    ))
    .unwrap()
}
#[tokio::test]
async fn endpoint_source_is_pinned_and_transport_failures_block_promotion()
-> Result<()> {
    let server = support::Server::new(true)?;
    let dataset = Dataset::memory()?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    let collector = SparqlEndpointSource::new(
        "urn:project",
        "urn:source",
        source(&server),
    )?;
    collector.collect(&w).await?;
    let old = dataset.snapshot_for("urn:project").await?;
    let quad = Quad::new(
        NamedNode::new("urn:ontology")?,
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        NamedNode::new("http://www.w3.org/2002/07/owl#Ontology")?,
        GraphName::DefaultGraph,
    );
    assert!(dataset.contains(&old, quad.clone()).await?);
    server.source(503, "do not leak this response");
    assert!(collector.collect(&w).await.is_err());
    assert!(dataset.ensure_ready("urn:project").await.is_err());
    assert!(dataset.contains(&old, quad).await?);
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn source_byte_limit_is_enforced_without_promoting_partial_data()
-> Result<()> {
    let server = support::Server::new(true)?;
    let mut config = source(&server);
    config.max_source_bytes = 1;
    let collector =
        SparqlEndpointSource::new("urn:project", "urn:source", config)?;
    let dataset = Dataset::memory()?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    assert!(collector.collect(&w).await.is_err());
    assert!(dataset.ensure_ready("urn:project").await.is_err());
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn external_store_verifies_isolation_and_preserves_exported_pins()
-> Result<()> {
    let server = support::Server::new(true)?;
    let dataset = Dataset::sparql(server.config()).await?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    let collector = SparqlEndpointSource::new(
        "urn:project",
        "urn:source",
        source(&server),
    )?;
    collector.collect(&w).await?;
    let old = dataset.snapshot_for("urn:project").await?;
    let temp = tempfile::tempdir()?;
    let archive = temp.path().join("archive.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive.clone()).await?;
    let quad = Quad::new(
        NamedNode::new("urn:ontology")?,
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        NamedNode::new("http://www.w3.org/2002/07/owl#Ontology")?,
        GraphName::DefaultGraph,
    );
    assert!(restored.contains(&old, quad.clone()).await?);
    let other_server = support::Server::new(true)?;
    let remote_restored = Dataset::sparql(other_server.config()).await?;
    remote_restored.import(archive).await?;
    assert!(remote_restored.contains(&old, quad).await?);
    drop(w);
    worker.await??;
    drop(dataset);
    assert_eq!(server.transaction_count(), 0);
    Ok(())
}
#[tokio::test]
async fn downgraded_snapshot_and_escaped_transaction_locations_are_rejected()
-> Result<()> {
    let weak = support::Server::new(false)?;
    let error = Dataset::sparql(weak.config()).await.err().unwrap();
    assert!(format!("{error:#}").contains("SERIALIZABLE"));
    let escaped = support::Server::new(true)?;
    escaped.bad_location();
    assert!(Dataset::sparql(escaped.config()).await.is_err());
    Ok(())
}
#[tokio::test]
async fn ambiguous_commit_poisoning_prevents_automatic_retries() -> Result<()> {
    let server = support::Server::new(true)?;
    let dataset = Dataset::sparql(server.config()).await?;
    server.lose_commit();
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    let collector = SparqlEndpointSource::new(
        "urn:project",
        "urn:source",
        source(&server),
    )?;
    assert!(collector.collect(&w).await.is_err());
    let error = dataset.snapshot_for("urn:project").await.err().unwrap();
    assert!(format!("{error:#}").contains("outcome is unknown"));
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn malformed_rdf_aborts_its_partial_contribution() -> Result<()> {
    let server = support::Server::new(true)?;
    server.source(
        200,
        "<urn:good> <urn:predicate> <urn:object> .\nthis is not RDF",
    );
    let dataset = Dataset::memory()?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    assert!(
        SparqlEndpointSource::new(
            "urn:project",
            "urn:source",
            source(&server)
        )?
        .collect(&w)
        .await
        .is_err()
    );
    assert!(dataset.ensure_ready("urn:project").await.is_err());
    let snapshot = dataset.snapshot_for("urn:project").await?;
    assert!(
        !dataset
            .contains(
                &snapshot,
                Quad::new(
                    NamedNode::new("urn:good")?,
                    NamedNode::new("urn:predicate")?,
                    NamedNode::new("urn:object")?,
                    GraphName::DefaultGraph
                )
            )
            .await?
    );
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn standard_query_endpoints_cannot_silently_become_writable_backends()
-> Result<()> {
    let server = support::Server::new(true)?;
    let mut config = server.config();
    config.transactions_endpoint = format!("{}/source", server.base);
    assert!(Dataset::sparql(config).await.is_err());
    Ok(())
}
#[tokio::test]
async fn remote_archive_limit_removes_incomplete_export() -> Result<()> {
    let server = support::Server::new(true)?;
    server.source(
        200,
        &format!(
            "<urn:large> <urn:predicate> \"{}\" .",
            "x".repeat(6000)
        ),
    );
    let mut config = server.config();
    config.http.max_response_bytes = 4096;
    let dataset = Dataset::sparql(config).await?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    SparqlEndpointSource::new("urn:project", "urn:source", source(&server))?
        .collect(&w)
        .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("archive.nq");
    assert!(dataset.export(archive.clone()).await.is_err());
    assert!(!archive.exists());
    assert_eq!(std::fs::read_dir(output.path())?.count(), 0);
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn cancelling_a_result_stream_rolls_back_its_read_transaction()
-> Result<()> {
    let server = support::Server::new(true)?;
    let dataset = Dataset::sparql(server.config()).await?;
    let values = (0..100)
        .map(|n| format!("<urn:value:{n}>"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut rows = dataset
        .select(format!(
            "SELECT ?value WHERE {{ VALUES ?value {{ {values} }} }}"
        ))
        .await?;
    assert!(rows.next().await?.is_some());
    drop(rows);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while server.transaction_count() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}
#[tokio::test]
async fn malformed_import_rolls_back_all_remote_statements() -> Result<()> {
    let server = support::Server::new(true)?;
    let dataset = Dataset::sparql(server.config()).await?;
    let temp = tempfile::tempdir()?;
    let archive = temp.path().join("invalid.nq");
    std::fs::write(
        &archive,
        "<urn:first> <urn:p> <urn:o> <urn:graph> .\ninvalid",
    )
    .unwrap();
    assert!(dataset.import(archive).await.is_err());
    let output = temp.path().join("empty.nq");
    dataset.export(output.clone()).await?;
    assert_eq!(std::fs::metadata(output)?.len(), 0);
    assert_eq!(server.transaction_count(), 0);
    Ok(())
}
#[tokio::test]
async fn source_timeout_fails_without_promoting_a_contribution() -> Result<()> {
    let server = support::Server::new(true)?;
    server.slow_source();
    let mut config = source(&server);
    config.http.timeout_seconds = 1;
    let dataset = Dataset::memory()?;
    let (w, worker) = GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    assert!(
        SparqlEndpointSource::new("urn:project", "urn:source", config)?
            .collect(&w)
            .await
            .is_err()
    );
    assert!(dataset.ensure_ready("urn:project").await.is_err());
    drop(w);
    worker.await??;
    Ok(())
}
#[tokio::test]
async fn bearer_credentials_are_read_from_the_child_environment() -> Result<()>
{
    let server = support::Server::new(true)?;
    let root = tempfile::tempdir()?;
    let config = root.path().join("source.yaml");
    std::fs::write(
        &config,
        format!(
            "endpoint: {}/source\nquery: CONSTRUCT {{ ?s ?p ?o }} WHERE {{ ?s \
             ?p ?o }}\nhttp:\n  authentication:\n    kind: bearer\n    \
             token_env: GRAPHARCH_TEST_TOKEN\n",
            server.base
        ),
    )?;
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args([
            "publication",
            "collect",
            "--project",
            "urn:project",
            "--collector",
            "urn:source",
            "--kind",
            "sparql-endpoint",
            "--source",
        ])
        .arg(config)
        .arg("--store")
        .arg(root.path().join("store"))
        .env("GRAPHARCH_TEST_TOKEN", "grapharch-test-only")
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(server.authenticated());
    assert!(
        !String::from_utf8_lossy(&output.stdout)
            .contains("grapharch-test-only")
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr)
            .contains("grapharch-test-only")
    );
    Ok(())
}
