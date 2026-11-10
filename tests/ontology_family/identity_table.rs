use super::*;
#[tokio::test]
async fn subject_identity_and_original_labels_belong_to_the_table_after_restore()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/ontology.ttl"),r#"
@prefix ex:<https://example.org/vocab/> .
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos:<http://www.w3.org/2004/02/skos/core#> .
ex: a owl:Ontology; rdfs:label "Identity fixture" .
ex:Party a owl:Class; rdfs:label "authorized party"@en; skos:prefLabel "authorised party"@en; skos:definition "A party with authorization."@en .
"#).await?;
    let built = collect(source.path()).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("retained.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    assert!(
        html.find("A party with authorization.").unwrap() <
            html.find("<table class=\"rdf-properties\"").unwrap()
    );
    assert!(
        doc.select(&scraper::Selector::parse("h1,h2,h3,h4").unwrap())
            .any(|heading| {
                heading
                    .text()
                    .collect::<String>()
                    .contains("authorised party")
            })
    );
    let table = doc
        .select(&scraper::Selector::parse("table.rdf-properties").unwrap())
        .find(|t| t.text().collect::<String>().contains("ex:Party"))
        .expect("identity attached to RDF table");
    let rows = table
        .select(&scraper::Selector::parse("thead > tr").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    for (row, value) in rows[..2]
        .iter()
        .zip(["ex:Party", "https://example.org/vocab/Party"])
    {
        let cells = row
            .select(&scraper::Selector::parse("td[colspan='2']").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].text().collect::<String>(), value);
        assert_eq!(
            cells[0].value().attr("style"),
            Some("text-align:center")
        );
    }
    let text = table.text().collect::<String>();
    assert!(text.contains("rdfs:label") && text.contains("skos:prefLabel"));
    assert!(
        !doc.select(&scraper::Selector::parse("p").unwrap())
            .any(|p| {
                ["Label:", "Identifier:", "IRI:"].iter().any(|label| {
                    p.text().collect::<String>().starts_with(label)
                })
            })
    );
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("table.cell(colspan: 2, align: center)"));
    assert!(typst.contains("header-row: 2"));
    assert!(!typst.contains("Identifier:") && !typst.contains("IRI:"));
    if let Ok(review) = std::env::var("GRAPHARCH_TABLE_REVIEW") {
        tokio::fs::create_dir_all(&review).await?;
        for name in ["index.html", "main.typ", "document.pdf"] {
            tokio::fs::copy(
                publication.directory.join(name),
                Path::new(&review).join(name),
            )
            .await?;
        }
    }
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
