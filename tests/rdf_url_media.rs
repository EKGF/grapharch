#[path = "support/http.rs"]
mod http;
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfUrl},
        dataset::Dataset,
        writer::GraphWriter,
    },
    tokio::{io::AsyncWriteExt, net::TcpListener},
};

#[tokio::test]
async fn explicitly_typed_rdf_accepts_neutral_transport_but_not_conflicting_formats()
-> Result<()> {
    let rdf = r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="urn:s"><label xmlns="urn:test:">Original label</label></rdf:Description></rdf:RDF>"#;
    for (mime, body, accepted) in [
        ("text/plain; charset=utf-8", rdf, true),
        ("application/octet-stream", rdf, true),
        ("application/rdf+xml", rdf, true),
        ("text/turtle", rdf, false),
        ("text/html", rdf, false),
        ("text/plain", "Not RDF/XML", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}/ontology.rdf", listener.local_addr()?);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            http::read_request_headers(&mut stream).await?;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: \
                         {mime}\r\nContent-Length: {}\r\nConnection: \
                         close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await?;
            anyhow::Ok(())
        });
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        let result = RdfUrl::new(
            "urn:project",
            "urn:collector",
            &url,
            "application/rdf+xml",
        )?
        .collect(&writer)
        .await;
        assert_eq!(result.is_ok(), accepted, "{mime}: {result:?}");
        server.await??;
        drop(writer);
        worker.await??;
        if accepted {
            let mut rows = dataset
                .select(
                    "SELECT ?value WHERE {GRAPH ?g {<urn:s> <urn:test:label> \
                     ?value}} LIMIT 1"
                        .into(),
                )
                .await?;
            assert!(rows.next().await?.is_some());
        }
    }
    Ok(())
}
