use super::*;

const ONTOLOGY: &str = r#"
@prefix ex:<https://example.org/vocab/> .
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd:<http://www.w3.org/2001/XMLSchema#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
@prefix dc:<http://purl.org/dc/elements/1.1/> .
@prefix ann:<https://ekgf.org/ontology/annotation#> .
ex: a owl:Ontology; rdfs:label "Dublin Core fixture";
 sh:declare [ sh:prefix "orcid"; sh:namespace "http://orcid.org/"^^xsd:anyURI ] .
ex:Stamped a owl:Class; rdfs:label "Stamped"; rdfs:comment "A stamped class.";
 dc:creator <http://orcid.org/0000-0003-2309-5068>;
 dc:date "2020-04-08T00:00:01Z"^^xsd:dateTime .
ex:Dated a owl:Class; rdfs:label "Dated"; rdfs:comment "A dated class.";
 dc:date "2020-04-08"^^xsd:date .
ex:Offset a owl:Class; rdfs:label "Offset"; rdfs:comment "An offset class.";
 dc:date "2020-06-20T12:36:22.5+02:00"^^xsd:dateTime .
ex:Plain a owl:Class; rdfs:label "Plain"; rdfs:comment "A plain class.";
 dc:date "2021-01-02" .
ex:Enterprise a owl:NamedIndividual, ex:Stamped; rdfs:label "Enterprise";
 ann:markdownDescription """
## Enterprise use case

The Enterprise stereotype identifies the enterprise scope.

### Assignment detail

A structural root has no parent.
"""@en .
"#;

#[tokio::test]
async fn dublin_core_terms_compact_and_dates_render_readably_after_restore()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(
        source.path().join("sources/ontology.ttl"),
        ONTOLOGY,
    )
    .await?;
    let built = collect(source.path()).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("retained.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    let tables = doc
        .select(&scraper::Selector::parse("table.rdf-properties").unwrap())
        .map(|table| table.text().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(tables.contains("dc:creator"), "{tables}");
    assert!(tables.contains("dc:date"), "{tables}");
    assert!(
        !tables.contains("http://purl.org/dc/elements/1.1/"),
        "well-known Dublin Core element names must be compacted: {tables}"
    );
    assert!(
        tables.contains("orcid:0000-0003-2309-5068"),
        "additional source sh:declare bindings must be honoured: {tables}"
    );
    for time in [
        r#"<time datetime="2020-04-08T00:00:01Z">8 April 2020, 00:00:01 UTC</time>"#,
        r#"<time datetime="2020-04-08">8 April 2020</time>"#,
        r#"<time datetime="2020-06-20T12:36:22.5+02:00">20 June 2020, 12:36:22.5 UTC+02:00</time>"#,
    ] {
        assert!(html.contains(time), "missing {time}");
    }
    assert!(
        tables.contains("2021-01-02") && !tables.contains("2 January 2021"),
        "an untyped string is not reinterpreted as a date"
    );
    let headings = scraper::Selector::parse("h1,h2,h3,h4,h5,h6").unwrap();
    for text in ["Enterprise use case", "Assignment detail"] {
        assert!(
            !doc.select(&headings)
                .any(|h| h.text().collect::<String>().contains(text)),
            "term description headings must not become sections: {text}"
        );
        assert!(
            html.contains(&format!("<strong>{text}</strong>")),
            "term description heading text must be retained: {text}"
        );
    }
    assert!(
        doc.select(&scraper::Selector::parse("nav").unwrap())
            .all(|nav| {
                !nav.text()
                    .collect::<String>()
                    .contains("Enterprise use case")
            }),
        "term description headings must not enter the outline"
    );
    let typst = tokio::fs::read_to_string(publication.typst_source()).await?;
    for text in [
        "8 April 2020, 00:00:01 UTC",
        "8 April 2020",
        "20 June 2020, 12:36:22.5 UTC+02:00",
        "dc:date",
        "dc:creator",
    ] {
        assert!(typst.contains(text), "PDF source misses {text}");
    }
    assert!(!typst.contains("http://purl.org/dc/elements/1.1/date"));
    assert!(
        typst
            .lines()
            .filter(|line| line.contains("Enterprise use case"))
            .all(|line| !line.contains("heading(")),
        "term description headings must not become PDF headings or bookmarks"
    );
    assert!(
        tokio::fs::read(publication.pdf().expect("published PDF edition"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
