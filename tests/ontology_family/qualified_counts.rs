use super::*;

#[tokio::test]
async fn maximum_count_projection_preserves_zero_after_source_removal()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("sources")).await?;
    tokio::fs::write(source.path().join("project.ttl"), DEFINITIONS).await?;
    tokio::fs::write(source.path().join("sources/ontology.ttl"),r#"
@prefix ex:<https://example.org/vocab/> .
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
ex: a owl:Ontology; rdfs:label "Qualified maximum fixture" .
ex:Period a owl:Class; rdfs:label "Period"; rdfs:subClassOf
 [a owl:Restriction; owl:onProperty ex:start; owl:onClass ex:Date; owl:maxQualifiedCardinality 1],
 [a owl:Restriction; owl:onProperty ex:finish; owl:onClass ex:Date; owl:maxQualifiedCardinality 1],
 [a owl:Restriction; owl:onProperty ex:duration; owl:onClass ex:Duration; owl:maxQualifiedCardinality 1] .
ex:NoDate a owl:Class; rdfs:label "No date"; rdfs:subClassOf [a owl:Restriction; owl:onProperty ex:start; owl:onClass ex:Date; owl:maxQualifiedCardinality 0] .
ex:Time a owl:Class; rdfs:label "Time"; rdfs:subClassOf [a owl:Restriction; owl:onProperty ex:value; owl:onDataRange <http://www.w3.org/2001/XMLSchema#string>; owl:qualifiedCardinality 1] .
ex:value a owl:DatatypeProperty; rdfs:label "has time value" .
ex:Merchant a owl:Class; rdfs:label "Merchant"; rdfs:subClassOf [a owl:Restriction; owl:onProperty ex:identifiedBy; owl:someValuesFrom ex:MerchantIdentifier] .
ex:MerchantIdentifier a owl:Class; rdfs:label "Merchant identifier" . ex:identifiedBy rdfs:label "is identified by" .
ex:Date a owl:Class; rdfs:label "Date" . ex:Duration a owl:Class; rdfs:label "Duration" .
ex:start a owl:ObjectProperty; rdfs:label "has start date" .
ex:finish a owl:ObjectProperty; rdfs:label "has end date" .
ex:duration a owl:ObjectProperty; rdfs:label "has duration" .
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
        panic!("publication required");
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    let figures = scraper::Selector::parse("figure.owl-class-diagram").unwrap();
    let captions = scraper::Selector::parse("figcaption").unwrap();
    let texts = scraper::Selector::parse("svg text").unwrap();
    for (class, count, instances) in
        [("Period", "0..1", 3), ("No date", "0..0", 1)]
    {
        let f = document
            .select(&figures)
            .find(|f| {
                f.select(&captions).any(|c| {
                    c.text()
                        .collect::<String>()
                        .starts_with(&format!("Class neighborhood: {class} "))
                })
            })
            .unwrap();
        let visible =
            f.select(&texts).flat_map(|n| n.text()).collect::<Vec<_>>();
        assert_eq!(
            visible.iter().filter(|s| **s == count).count(),
            instances
        );
        assert!(!visible.iter().any(|s| s.contains("expression")));
    }
    let time = document
        .select(&figures)
        .find(|f| {
            f.select(&captions).any(|c| {
                c.text()
                    .collect::<String>()
                    .starts_with("Class neighborhood: Time ")
            })
        })
        .unwrap();
    let time_text = time
        .select(&texts)
        .flat_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        time_text.contains("has time value : xsd:string [1]"),
        "{time_text}"
    );
    assert!(!time_text.contains("OWL expression"));
    let merchant = document
        .select(&figures)
        .find(|f| {
            f.select(&captions).any(|c| {
                c.text()
                    .collect::<String>()
                    .starts_with("Class neighborhood: Merchant ")
            })
        })
        .unwrap();
    let merchant_text = merchant
        .select(&texts)
        .flat_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        merchant_text.contains("is identified by") &&
            merchant_text.contains("1..*")
    );
    assert!(!merchant_text.contains("OWL expression"));
    assert!(html.contains("onDataRange"));
    assert!(html.contains("maxQualifiedCardinality"));
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    if let Some(directory) =
        std::env::var_os("GRAPHARCH_QUALIFIED_MAXIMUM_OUTPUT")
    {
        let destination = std::path::PathBuf::from(directory);
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "main.typ", "document.pdf", "style.css"] {
            tokio::fs::copy(
                publication.directory.join(name),
                destination.join(name),
            )
            .await?;
        }
    }
    Ok(())
}
