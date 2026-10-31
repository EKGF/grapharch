use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    },
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    },
};

#[tokio::test]
async fn http_family_showcase_renders_after_sources_disappear() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/ontology.ttl", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = [0; 4096];
        stream.read(&mut request).await?;
        let body = r##"@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
<https://example.org/terms/> a owl:Ontology; rdfs:label "Example ontology"@en; dct:description "Remote source example."@en .
<https://example.org/terms/Thing> a owl:Class; rdfs:label "Example thing"@en; dct:description "A documented thing."@en; rdfs:isDefinedBy <https://example.org/terms/> ."##;
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: \
                     text/turtle\r\nContent-Length: {}\r\nConnection: \
                     close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await?;
        anyhow::Ok(())
    });
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("project.ttl"), r##"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:http:project> a ga:Project; dct:title "HTTP family"; ga:target <urn:http:target> .
<urn:http:scope> a ga:Scope .
<urn:http:profile> a ga:Profile; pub:publishesFamily <urn:http:family> .
<urn:http:family> a gov:Family; rdfs:label "HTTP family" .
<urn:http:target> a ga:Target; dct:title "HTTP family"; dct:description "A remotely collected ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer "specification"; ga:destination "specification"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:publisher> a gov:Authority; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://example.org/terms/> gov:inFamily <urn:http:family>; sh:declare [sh:prefix "ex"; sh:namespace "https://example.org/terms/"] .
"##).await?;
    tokio::fs::write(
        source.path().join("showcase.yaml"),
        format!(
            "definitions: project.ttl\nproject: urn:http:project\ntarget: \
             urn:http:target\nsource:\n  kind: rdf-urls\n  documents:\n    - \
             url: {url}\n      media_type: text/turtle\n"
        ),
    )
    .await?;
    let cache = tempfile::tempdir()?;
    let (dataset, plans) =
        showcases::family(&source.path().join("showcase.yaml"), cache.path())
            .await?;
    server.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(
        html.contains("Example thing") && html.contains("A documented thing.")
    );
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
