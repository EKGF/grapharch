#[path = "support/http.rs"]
mod http;
#[path = "support/showcase_rdf.rs"]
mod showcase_rdf;

use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    tokio::{io::AsyncWriteExt, net::TcpListener},
};

#[tokio::test]
async fn http_family_showcase_renders_after_sources_disappear() -> Result<()> {
    let (html, _) = family_publication("").await?;
    assert!(
        html.contains("Example thing") && html.contains("A documented thing.")
    );
    Ok(())
}

#[tokio::test]
async fn formal_family_profile_nests_chapters_and_omits_review_material()
-> Result<()> {
    let (_, typst) = family_publication(r#"
<urn:http:profile> ga:w3cFrontMatter false ; ga:familyOverview false ;
    ga:coverageAppendix false ; ga:familyPlacement <urn:http:clause> ; ga:familyBaseOrder 11 ;
    ga:placement <urn:http:clause> .
<urn:http:clause> a ga:Placement ; ga:component <urn:http:clause-component> ; ga:order 10 .
<urn:http:clause-component> a ga:Component ; dct:title "Ontologies" ; ga:fragment <urn:http:clause-body> .
<urn:http:clause-body> a ga:Fragment ; ga:subject <urn:http:edition> ; ga:contribution <urn:http:authored> ;
    ga:format "text/markdown" ; ga:body "The library comprises these ontologies." .
<urn:http:scope> ga:include <urn:http:edition> .
<urn:http:definitions> a ga:Processor .
<urn:http:authored> a ga:Contribution ; ga:processor <urn:http:definitions> ; ga:project <urn:http:project> ;
    ga:source "project.ttl" ; ga:revision "1" .
"#).await?;
    assert!(!typst.contains("[#text(\"Abstract\")]"));
    assert!(!typst.contains("Status of This Document"));
    assert!(!typst.contains("Documentation coverage"));
    assert!(!typst.contains("Explore the"));
    let clause = typst
        .find("#heading(level: 1)[#text(\"Ontologies\")]")
        .expect("authored clause");
    let chapter = typst
        .find("#heading(level: 2)[#text(\"Example ontology\")]")
        .expect("nested chapter");
    assert!(clause < chapter);
    Ok(())
}

async fn family_publication(extra: &str) -> Result<(String, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/ontology.ttl", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        http::read_request_headers(&mut stream).await?;
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
    tokio::fs::write(source.path().join("project.ttl"), format!(r##"
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
<urn:http:target> a ga:Target; dct:title "HTTP family"; dct:description "A remotely collected ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer>; ga:destination "specification"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:review> a ga:Target; dct:title "HTTP family"; dct:description "A remotely collected ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer>; ga:destination "review"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:publisher> a gov:Authority; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://www.omg.org/spec/Commons/Classifiers/> gov:inFamily <urn:http:family>; sh:declare [sh:prefix "cmns-cls"; sh:namespace "https://www.omg.org/spec/Commons/Classifiers/"] .
{extra}
"##)).await?;
    tokio::fs::write(
        source.path().join("showcase.ttl"),
        format!(
            "@prefix ga: <https://ekgf.org/ontology/grapharch#> \
             .\n<urn:test:showcase> a ga:Showcase ; ga:definitionsPath \
             \"project.ttl\" ; ga:showcaseProject <urn:http:project> ; \
             ga:showcaseTargets ( <urn:http:target> <urn:http:review> ) ; \
             ga:rdfDocument <{url}> .\n<{url}> ga:format \"text/turtle\" .\n"
        ),
    )
    .await?;
    let cache = tempfile::tempdir()?;
    let (dataset, plans) =
        showcases::family(&source.path().join("showcase.ttl"), cache.path())
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
    let typst =
        tokio::fs::read_to_string(publications[1].directory.join("main.typ"))
            .await?;
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok((html, typst))
}

#[tokio::test]
async fn published_commons_inventory_uses_authoritative_urls_without_a_copied_corpus()
-> Result<()> {
    let documents =
        showcase_rdf::documents("examples/omg-commons/showcase.ttl")?;
    assert_eq!(documents.len(), 24);
    for (url, format) in &documents {
        assert!(
            url.starts_with("https://www.omg.org/spec/Commons/") &&
                url.ends_with(".rdf")
        );
        assert_eq!(format, "application/rdf+xml");
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
        std::path::Path::new("examples/omg-commons/showcase.ttl"),
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
    let typst = tokio::fs::read_to_string(root.join("pdf/main.typ")).await?;
    assert!(typst.contains("// Theme: urn:omg-commons:theme:theme"));
    assert!(typst.contains("#image(\"cover-logo.svg\""));
    assert!(
        typst.contains("Copyright © 2022-2026, Object Management Group, Inc.")
    );
    assert!(typst.contains("[#text(\"Annex A: Deliverables\")]"));
    assert!(!typst.contains("Status of This Document"));
    assert!(typst.contains("#set page(numbering: \"i\")"));
    assert!(typst.contains("#set page(numbering: \"1\")"));
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
