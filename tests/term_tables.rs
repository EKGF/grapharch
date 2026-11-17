//! Standards-style tabular ontology references (`ga:termTable`), regional
//! English labels, and plain-text ontology introductions.
#[path = "support/http.rs"]
mod http;

use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    tokio::{io::AsyncWriteExt, net::TcpListener},
};

const PREFIXES: &str = r#"@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdf:<http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix skos:<http://www.w3.org/2004/02/skos/core#> .
@prefix xsd:<http://www.w3.org/2001/XMLSchema#> .
@prefix ex:<https://example.org/ont/> .
@prefix other:<https://example.org/other/> .
"#;

const ONTOLOGY: &str = r#"
<https://example.org/ont/> a owl:Ontology ; rdfs:label "Example Ontology" ;
    dct:abstract "\t\tThis ontology defines\n\t\tauthorizations.\n\t\t\n\t\tIt is an example." ;
    dct:license "Copyright (c) 2026 Alpha Inc.\n\t\tCopyright (c) 2026 Beta LLC\n\t\t\n\t\tPermission is hereby granted." ;
    owl:versionIRI <https://example.org/ont/1/> ;
    owl:imports <https://example.org/other/> .
ex:Authorization a owl:Class ; rdfs:label "authorisation"@en-GB, "authorization"@en-US, "autorisation"@fr ;
    skos:definition "situation in which a party authorizes" ;
    skos:example "a power of attorney" ;
    rdfs:isDefinedBy <https://example.org/ont/> ;
    rdfs:subClassOf ex:Situation,
        [ a owl:Restriction ; owl:onProperty ex:hasParty ; owl:someValuesFrom ex:Party ] ,
        [ a owl:Restriction ; owl:onProperty ex:hasDate ; owl:maxQualifiedCardinality "1"^^xsd:nonNegativeInteger ; owl:onDataRange xsd:date ] ,
        [ a owl:Restriction ; owl:onProperty ex:hasParty ; owl:allValuesFrom [ a owl:Class ; owl:unionOf ( ex:Party ex:Situation ) ] ] ,
        [ a owl:Restriction ; owl:onProperty [ owl:inverseOf ex:isPartyOf ] ; owl:minCardinality "0"^^xsd:nonNegativeInteger ] ;
    owl:disjointWith ex:Party .
ex:Situation a owl:Class ; rdfs:label "situation"@en ; skos:definition "state of affairs" .
ex:Party a owl:Class ; rdfs:label "party" ; skos:definition "someone" ;
    owl:equivalentClass [ a owl:Class ; owl:intersectionOf ( ex:Situation [ a owl:Class ; owl:complementOf ex:Authorization ] ) ] .
ex:hasParty a owl:ObjectProperty, owl:FunctionalProperty ; rdfs:label "has party" ; skos:definition "relates a situation to a party" ;
    rdfs:domain ex:Authorization ; rdfs:range ex:Party ; owl:inverseOf ex:isPartyOf ; rdfs:subPropertyOf other:relates .
ex:isPartyOf a owl:ObjectProperty ; rdfs:label "is party of" ; skos:definition "relates a party to a situation" ;
    owl:propertyChainAxiom ( ex:hasParty ex:hasParty ) .
ex:hasDate a owl:DatatypeProperty ; rdfs:label "has date" ; skos:definition "relates something to a date" ; rdfs:range xsd:date .
ex:Text a rdfs:Datatype ; rdfs:label "text" ; skos:definition "string or language-tagged string" ;
    owl:equivalentClass [ a rdfs:Datatype ; owl:unionOf ( xsd:string rdf:langString ) ] .
ex:Day a owl:NamedIndividual, ex:Situation ; rdfs:label "day" ; skos:definition "one day" ;
    ex:hasDate "2026-01-01"^^xsd:date .
other:Thing a owl:Class ; owl:equivalentClass ex:Party .
other:Bare a owl:Class .
"#;

const TABLES: &str = r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix skos:<http://www.w3.org/2004/02/skos/core#> .
<urn:http:profile> ga:termTable <urn:http:tables> .
<urn:http:tables> a ga:TermTable ;
    ga:metadataCaption "{title} Metadata" ; ga:detailsCaption "{title} Details" ;
    ga:metadataLeadIn "Metadata for the {title} is given in {table}." ;
    ga:detailsLeadIn "The details of the {title} are provided in {table}, below." ;
    ga:metadataPredicates ( rdfs:label dct:abstract dct:license owl:versionIRI ) ;
    ga:annotationPredicates ( skos:definition skos:example ) ;
    ga:referenceSection <urn:http:classes>, <urn:http:datatypes>, <urn:http:individuals>, <urn:http:properties> .
<urn:http:classes> dct:title "Classes" ; ga:order 1 ; ga:termType owl:Class .
<urn:http:datatypes> dct:title "Datatypes" ; ga:order 2 ; ga:termType rdfs:Datatype .
<urn:http:individuals> dct:title "Individuals" ; ga:order 3 ; ga:termType owl:NamedIndividual .
<urn:http:properties> dct:title "Properties" ; ga:order 4 ;
    ga:termType owl:ObjectProperty, owl:DatatypeProperty, owl:AnnotationProperty .
skos:definition rdfs:label "definition"@en .
skos:example rdfs:label "example"@en .
"#;

#[tokio::test]
async fn regional_english_labels_name_terms_without_missing_label_warnings()
-> Result<()> {
    let (html, _) = publish(
        r#"<https://example.org/ont/> a owl:Ontology ; rdfs:label "Example Ontology" .
ex:Authorization a owl:Class ; rdfs:label "authorisation"@en-GB, "authorization"@en-US ;
    skos:definition "situation in which a party authorizes" ."#,
        "",
    )
    .await?;
    assert!(
        html.contains("authorisation; authorization"),
        "{html}"
    );
    assert!(!html.contains("No English or language-neutral label"));
    Ok(())
}

#[tokio::test]
async fn tab_indented_ontology_introductions_are_prose_not_code() -> Result<()>
{
    let (html, typst) = publish(ONTOLOGY, "").await?;
    assert!(html.contains("<p>This ontology defines authorizations.</p>"));
    assert!(html.contains("<p>It is an example.</p>"));
    assert!(!typst.contains("This ontology defines\n"));
    Ok(())
}

#[tokio::test]
async fn term_tables_render_metadata_and_description_logic_axioms() -> Result<()>
{
    let (html, typst) = publish(ONTOLOGY, TABLES).await?;
    let text = visible(&html);
    // Metadata: the ontology IRI, then the configured header predicates in
    // order, licence lines kept; owl:imports is not configured.
    for expected in [
        "Metadata for the Example Ontology is given in Example Ontology Metadata.",
        "Metadata Term Value Ontology IRI https://example.org/ont/ rdfs:label \
         Example Ontology dct:abstract",
        "dct:license Copyright (c) 2026 Alpha Inc. Copyright (c) 2026 Beta LLC \
         Permission is hereby granted. owl:versionIRI https://example.org/ont/1/",
    ] {
        assert!(text.contains(expected), "{expected}\n{text}");
    }
    assert!(html.contains("Copyright (c) 2026 Alpha Inc.<br />"));
    assert!(!text.contains("owl:imports"));
    // Details: sections in order, rows by name, regional labels with tags,
    // configured annotations, and description-logic axioms.
    for expected in [
        "The details of the Example Ontology are provided in Example Ontology \
         Details, below.",
        "Classes Name Annotations Class Expressions Authorization \
         (authorisation@en-GB, authorization@en-US) Definition: situation in \
         which a party authorizes Example: a power of attorney Parent Class: \
         Situation Property Restriction: ∃ hasParty.Party Property \
         Restriction: ≥ 0 isPartyOf⁻ Property Restriction: ∀ hasParty (Party \
         ⊔ Situation) Property Restriction: ≤ 1 hasDate.xsd:date Class Axiom: \
         ¬ Party other:Thing Equivalent Class: Party Party (party) \
         Definition: someone Class Axiom: ≡ Situation ⊓ ¬ Authorization \
         Situation (situation@en)",
        "Datatypes Name Annotations Class Expressions Text (text) Definition: \
         string or language-tagged string Equivalent Datatype: xsd:string ⊔ \
         rdf:langString",
        "Individuals Name Annotations Individual Axioms Day (day) Definition: \
         one day Type: Situation hasDate = \"2026-01-01\"^^xsd:date",
        "Properties Name Annotations Property Axioms hasDate (has date) \
         Definition: relates something to a date Range: xsd:date hasParty \
         (has party) Definition: relates a situation to a party Type: \
         owl:FunctionalProperty Parent Property: other:relates Domain: \
         Authorization Range: Party Inverse: isPartyOf isPartyOf (is party \
         of) Definition: relates a party to a situation Property Chain: \
         hasParty ∘ hasParty",
    ] {
        assert!(text.contains(expected), "{expected}\n{text}");
    }
    // A foreign subject needs a statement beyond its declaration; French
    // labels and rdfs:isDefinedBy are not shown.
    assert!(!text.contains("other:Bare"));
    assert!(!text.contains("autorisation"));
    assert!(!text.contains("Not declared in collected sources"));
    // Term links resolve to their rows; the foreign row has no destination.
    let row = html
        .split("<a href=\"#")
        .skip(1)
        .find_map(|tail| {
            tail.split_once("\">Party</a>")
                .map(|(anchor, _)| anchor.to_owned())
        })
        .expect("a link to the Party row");
    assert!(row.contains("-h-row-"), "{row}");
    assert!(html.contains(&format!("<span id=\"{row}\"></span>")));
    // The PDF numbers both tables and references them by number.
    assert_eq!(
        typst
            .matches("#figure(kind: table, supplement: [Table]")
            .count(),
        2
    );
    assert_eq!(typst.matches("#ref(label(\"ga-table-").count(), 2);
    assert!(typst.contains("#text(\"Example Ontology Details\")"));
    assert!(typst.contains("cell-spacing: 0.8em"));
    Ok(())
}

/// The visible text of an HTML page: block boundaries become spaces, inline
/// markup joins its neighbours, and whitespace collapses.
fn visible(html: &str) -> String {
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            break
        };
        let name = rest[start + 1..start + end]
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or_default();
        if matches!(
            name,
            "p" | "td" |
                "th" |
                "tr" |
                "br" |
                "li" |
                "div" |
                "figcaption" |
                "h2" |
                "h3" |
                "table" |
                "thead" |
                "tbody"
        ) {
            text.push(' ');
        }
        rest = &rest[start + end + 1..];
    }
    text.push_str(rest);
    text.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn publish(ontology: &str, extra: &str) -> Result<(String, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/ontology.ttl", listener.local_addr()?);
    let body = format!("{PREFIXES}{ontology}");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        http::read_request_headers(&mut stream).await?;
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
<urn:http:project> a ga:Project; dct:title "Table family"; ga:target <urn:http:target>, <urn:http:review> .
<urn:http:scope> a ga:Scope .
<urn:http:profile> a ga:Profile; pub:publishesFamily <urn:http:family> .
<urn:http:family> a gov:Family; rdfs:label "Table family" .
<urn:http:target> a ga:Target; dct:title "Table family"; dct:description "A tabular ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "target"; ga:destination "specification"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:review> a ga:Target; dct:title "Table family"; dct:description "A tabular ontology reference."; rdfs:comment "Unofficial test preview."; ga:scope <urn:http:scope>; ga:profile <urn:http:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "review"; ga:destination "review"; ga:publisher <urn:http:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:http:publisher> a gov:Authority ; <https://ekgf.org/ontology/grapharch#slug> "example"; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://example.org/ont/> gov:inFamily <urn:http:family>;
    sh:declare [sh:prefix "ex"; sh:namespace "https://example.org/ont/"], [sh:prefix "other"; sh:namespace "https://example.org/other/"] .
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
    server.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications = showcases::publish(
        &Publisher::new(restored)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans,
        output.path(),
    )
    .await?;
    let html =
        tokio::fs::read_to_string(publications[0].directory.join("index.html"))
            .await?;
    let typst =
        tokio::fs::read_to_string(publications[1].directory.join("main.typ"))
            .await?;
    assert!(
        tokio::fs::read(publications[1].pdf().expect("published PDF edition"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok((html, typst))
}
