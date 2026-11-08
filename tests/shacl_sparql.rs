use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::{NamedNode, Term},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};
const SHAPE: &str =
    "https://ekgf.org/dprod/spec/develop/contracts/shapes/ScheduleShape";
#[tokio::test]
async fn schedule_queries_retain_code_lines_in_restored_html_and_pdf()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    let path = source.path().join("schedule.ttl");
    let original = include_str!("fixtures/shacl-sparql/dprod-schedule.ttl");
    tokio::fs::write(&path, original).await?;
    RdfFile::new("urn:p", "urn:shape", path)?
        .collect(&writer)
        .await?;
    let ask = "\r\n  ASK {\r\n    $this ?predicate ?object .\r\n  }\r\n";
    let path = source.path().join("ask.ttl");
    tokio::fs::write(
        &path,
        format!(
            "@prefix sh:<http://www.w3.org/ns/shacl#>. <urn:ask-validator> a sh:SPARQLAskValidator; <http://www.w3.org/2000/01/rdf-schema#label> \"ASK validator\"; <http://www.w3.org/2000/01/rdf-schema#comment> \"An original formatting regression.\"; sh:ask {}; <https://example.org/{}> \"A long predicate boundary fixture.\".",
            oxrdf::Literal::new_simple_literal(ask),
            "long-predicate-".repeat(30)
        ),
    )
    .await?;
    RdfFile::new("urn:p", "urn:ask", path)?
        .collect(&writer)
        .await?;
    let doc = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:p").await?,
        "urn:p",
        NamedNode::new(SHAPE)?,
        "en",
        Arc::new(BTreeMap::from([
            ("http://www.w3.org/ns/shacl#".into(), "sh".into()),
            (
                "http://www.w3.org/2000/01/rdf-schema#".into(),
                "rdfs".into(),
            ),
            ("http://purl.org/dc/terms/".into(), "dct".into()),
            (
                "http://www.w3.org/2001/XMLSchema#".into(),
                "xsd".into(),
            ),
            (
                "https://ekgf.org/dprod/spec/develop/contracts/shapes/".into(),
                "dprod-shapes".into(),
            ),
            (
                "https://ekgf.org/dprod/spec/develop/".into(),
                "dprod".into(),
            ),
        ])),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    doc.document(&writer).await?;
    let ask_doc = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:p").await?,
        "urn:p",
        NamedNode::new("urn:ask-validator")?,
        "en",
        Arc::new(BTreeMap::new()),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    ask_doc.document(&writer).await?;

    let path = source.path().join("project.ttl");
    tokio::fs::write(&path,format!(r##"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:t> <http://purl.org/dc/terms/title> "SHACL SPARQL constraints";a ga:Target;ga:renderer "specification";ga:destination "specification";ga:publisher <urn:authority>;ga:publicationStatus "Test Draft";ga:publicationDate "2026-11-07"^^<http://www.w3.org/2001/XMLSchema#date>;ga:thisVersion <https://example.org/shacl/test/>;ga:latestVersion <https://example.org/shacl/>;ga:license <https://creativecommons.org/licenses/by/4.0/>;ga:scope <urn:s>;ga:profile <urn:f>. <urn:authority> a <https://ekgf.org/ontology/artifact-governance#Authority>; <http://www.w3.org/2000/01/rdf-schema#label> "Example standards group";ga:shortName "EX";ga:url "https://example.org/";ga:accentColor "#2b5f9f". <urn:s> a ga:Scope;ga:include <{SHAPE}>,<urn:ask-validator>,<urn:abstract-subject>,<urn:status-subject>. <urn:f> a ga:Profile;ga:placement <urn:abstract-placement>,<urn:status-placement>,<urn:placement>,<urn:ask-placement>. <urn:placement> a ga:Placement;ga:component <urn:c>;ga:order 2. <urn:c> a ga:Component;ga:fragment {}.
<urn:ask-placement> a ga:Placement;ga:component <urn:ask-component>;ga:order 3. <urn:ask-component> a ga:Component;ga:fragment {}.
<urn:definitions> a ga:Collector;ga:kind "rdf-file";ga:source "project.ttl".
<urn:authored> a ga:Contribution;ga:project <urn:p>;ga:processor <urn:definitions>;ga:source "fixture";ga:revision "1".
<urn:abstract-placement> a ga:Placement;ga:component <urn:abstract-component>;ga:order 0;ga:sectionKind "abstract".
<urn:status-placement> a ga:Placement;ga:component <urn:status-component>;ga:order 1;ga:sectionKind "status".
<urn:abstract-component> a ga:Component;ga:fragment <urn:abstract>.
<urn:status-component> a ga:Component;ga:fragment <urn:status>.
<urn:abstract> a ga:Fragment;ga:subject <urn:abstract-subject>;ga:body "A formatting regression for retained SHACL queries.";ga:format "text/markdown";ga:contribution <urn:authored>.
<urn:status> a ga:Fragment;ga:subject <urn:status-subject>;ga:body "An original, non-normative regression fixture.";ga:format "text/markdown";ga:contribution <urn:authored>.
"##,doc.fragment_id(),ask_doc.fragment_id())).await?;
    RdfFile::new("urn:p", "urn:definitions", path)?
        .collect(&writer)
        .await?;
    let revision = Assembler::new(dataset.clone()).assemble("urn:p").await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:t")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let mut expected = Vec::new();
    let mut quads = oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(original.as_bytes());
    for quad in &mut quads {
        let quad = quad?;
        if quad.predicate.as_str() == "http://www.w3.org/ns/shacl#select" {
            let Term::Literal(value) = quad.object else {
                panic!("literal query")
            };
            expected.push(value.value().to_owned());
        }
    }
    expected.push(ask.into());
    let snapshot = restored.snapshot_for("urn:p").await?;
    let view = RevisionView::new(restored.clone(), snapshot.clone());
    let body = format!(
        "{}\n{}",
        view.text(&doc.fragment_id(), "body").await?,
        view.text(&ask_doc.fragment_id(), "body").await?
    );
    let predicate =
        Dataset::logical_pattern(&snapshot, "?constraint ?predicate ?query", 0);
    let mut retained = restored.select(format!("SELECT DISTINCT ?query WHERE {{ VALUES ?predicate {{ <http://www.w3.org/ns/shacl#select> <http://www.w3.org/ns/shacl#ask> }} {predicate} }}")).await?;
    let mut literals = std::collections::BTreeSet::new();
    while let Some(row) = retained.next().await? {
        let Term::Literal(query) = &row["query"] else {
            panic!("SHACL code literal")
        };
        literals.insert(query.value().to_owned());
    }
    assert_eq!(
        literals,
        expected.iter().cloned().collect(),
        "retained RDF literal bytes changed"
    );

    assert_eq!(
        body.matches("```sparql").count(),
        5,
        "every explicit SHACL query needs code semantics"
    );
    for query in &expected {
        assert!(
            body.replace("\r\n", "\n").contains(&display_query(query)),
            "generated code must remove outer layout and retain relative \
             indentation"
        );
    }
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let parsed = scraper::Html::parse_document(&html);
    let codes: Vec<_> = parsed
        .select(
            &scraper::Selector::parse("pre > code.language-sparql").unwrap(),
        )
        .map(|code| code.text().collect::<String>())
        .collect();
    assert_eq!(codes.len(), 5);
    assert_eq!(
        parsed
            .select(&scraper::Selector::parse("table.rdf-properties").unwrap())
            .count(),
        2,
        "RDF tables need predicate-specific layout"
    );
    let query_ids: Vec<_> = parsed
        .select(&scraper::Selector::parse("[id]").unwrap())
        .filter_map(|node| node.value().attr("id"))
        .filter(|id| id.contains("-sparql-query-"))
        .collect();
    assert_eq!(query_ids.len(), 5);
    assert_eq!(
        query_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5,
        "queries on different resources need distinct local targets"
    );

    assert!(
        parsed
            .select(&scraper::Selector::parse("td pre").unwrap())
            .next()
            .is_none(),
        "code blocks must not be squeezed into table cells"
    );
    for link in parsed.select(&scraper::Selector::parse("a[href]").unwrap()) {
        if let Some(target) = link
            .value()
            .attr("href")
            .and_then(|href| href.strip_prefix('#'))
            .filter(|id| id.contains("sparql-query-"))
        {
            assert!(
                parsed
                    .select(&scraper::Selector::parse("[id]").unwrap())
                    .any(|node| node.value().attr("id") == Some(target)),
                "missing query code target: {target}"
            );
        }
    }

    for query in &expected {
        assert!(
            codes
                .iter()
                .any(|code| { code.contains(&display_query(query)) }),
            "HTML flattened source query"
        );
    }
    assert!(
        parsed
            .select(
                &scraper::Selector::parse("code.language-sparql span[style]")
                    .unwrap()
            )
            .next()
            .is_some()
    );
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert_eq!(typst.matches("#ga-code((").count(), 5);
    assert!(
        typst.contains("columns: (30%, 1fr)"),
        "predicate tables need flexible value space in PDF"
    );
    assert!(
        tokio::fs::metadata(publication.directory.join("document.pdf"))
            .await?
            .len() >
            1000
    );
    if let Some(root) = std::env::var_os("GRAPHARCH_SHACL_PREVIEW") {
        let root = std::path::PathBuf::from(root);
        tokio::fs::create_dir_all(&root).await?;
        for file in &publication.manifest.files {
            let destination = root.join(file);
            if let Some(parent) = destination.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(publication.directory.join(file), destination)
                .await?;
        }
    }
    Ok(())
}

fn display_query(query: &str) -> String {
    let lines: Vec<_> = query.lines().collect();
    let first = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap();
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap();
    let indent = lines[first].len() - lines[first].trim_start().len();
    let mut display = lines[first..=last]
        .iter()
        .map(|line| line.strip_prefix(&" ".repeat(indent)).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    display.push('\n');
    display
}
