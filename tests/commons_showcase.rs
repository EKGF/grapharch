use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
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
<https://www.omg.org/spec/Commons/Classifiers/> a owl:Ontology; rdfs:label "Example ontology"@en; dct:description "Remote source example."@en .
<https://www.omg.org/spec/Commons/Classifiers/Thing> a owl:Class; rdfs:label "Example thing"@en; dct:description "A documented thing."@en; rdfs:isDefinedBy <https://www.omg.org/spec/Commons/Classifiers/> ."##;
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
<urn:http:project> a ga:Project; dct:title "HTTP family"; ga:target <urn:http:target>, <urn:http:review> .
<urn:http:scope> a ga:Scope .
<urn:http:profile> a ga:Profile; pub:publishesFamily <urn:http:family> .
<urn:http:family> a gov:Family; rdfs:label "HTTP family" .
<urn:http:target> a ga:Target; dct:title "HTTP family"; dct:description "A remotely collected ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer "specification"; ga:destination "specification"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:review> a ga:Target; dct:title "HTTP family"; dct:description "A remotely collected ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer "specification"; ga:destination "review"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:publisher> a gov:Authority; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://www.omg.org/spec/Commons/Classifiers/> gov:inFamily <urn:http:family>; sh:declare [sh:prefix "cmns-cls"; sh:namespace "https://www.omg.org/spec/Commons/Classifiers/"] .
"##).await?;
    tokio::fs::write(
        source.path().join("showcase.yaml"),
        format!(
            "definitions: project.ttl\nproject: urn:http:project\ntarget: \
             urn:http:target\nadditional_targets:\n  - \
             urn:http:review\nsource:\n  kind: rdf-urls\n  documents:\n    - \
             url: {url}\n      media_type: text/turtle\n"
        ),
    )
    .await?;
    let cache = tempfile::tempdir()?;
    let (dataset, plans) =
        showcases::family(&source.path().join("showcase.yaml"), cache.path())
            .await?;
    assert_eq!(plans.len(), 2);
    server.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output.path())
            .await?;
    assert_eq!(publications.len(), 2);
    let publication = &publications[0];
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

#[tokio::test]
async fn published_commons_inventory_uses_authoritative_urls_without_a_copied_corpus()
-> Result<()> {
    let config: serde_yaml::Value = serde_yaml::from_str(
        &tokio::fs::read_to_string("examples/omg-commons/showcase.yaml")
            .await?,
    )?;
    assert_eq!(
        config["source"]["kind"].as_str(),
        Some("rdf-urls")
    );
    let documents = config["source"]["documents"].as_sequence().unwrap();
    assert_eq!(documents.len(), 23);
    for document in documents {
        let url = document["url"].as_str().unwrap();
        assert!(
            url.starts_with("https://www.omg.org/spec/Commons/") &&
                url.ends_with(".rdf")
        );
        assert_eq!(
            document["media_type"].as_str(),
            Some("application/rdf+xml")
        );
    }
    assert!(!tokio::fs::try_exists("examples/omg-commons/sources").await?);
    Ok(())
}

#[tokio::test]
#[ignore = "live OMG source acceptance; requires network"]
async fn live_commons_showcase_from_restored_kg() -> Result<()> {
    let started = std::time::Instant::now();
    let root = std::path::Path::new("output/commons-review");
    tokio::fs::create_dir_all(root).await?;
    let (dataset, plans) = showcases::family(
        std::path::Path::new("examples/omg-commons/showcase.yaml"),
        std::path::Path::new("output/cache/git"),
    )
    .await?;
    let archive = root.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    assert_eq!(plans.len(), 2);
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, root).await?;
    let publication = &publications[0];
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("23 ontologies"));
    assert!(html.contains("Roles and Compositions"));
    assert!(
        tokio::fs::read(root.join("pdf/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    eprintln!(
        "Commons collection+restore+HTML/PDF: {:.2}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
