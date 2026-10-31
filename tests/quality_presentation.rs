use grapharch::publication::{
    assembly::Assembler,
    collect::{Collector, RdfFile},
    dataset::Dataset,
    plan::TargetPlanner,
    render::{PublishOutcome, Publisher},
    writer::GraphWriter,
};

struct RemoteIdentity {
    inner:  RdfFile,
    source: String,
}
#[async_trait::async_trait]
impl Collector for RemoteIdentity {
    fn identity(
        &self,
    ) -> anyhow::Result<grapharch::publication::collect::SourceIdentity<'_>>
    {
        Ok(grapharch::publication::collect::SourceIdentity {
            project:   "urn:specification:project",
            processor: &self.source,
            source:    &self.source,
        })
    }

    async fn produce(
        &self,
        output: &mut grapharch::publication::writer::ContributionStream,
    ) -> anyhow::Result<String> {
        self.inner.produce(output).await
    }
}

#[tokio::test]
async fn competing_values_are_readable_after_kg_restore() -> anyhow::Result<()>
{
    let sources = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    for (name, value) in [
        (
            "first",
            "First paragraph.\\nSecond paragraph <script> literal.",
        ),
        ("second", "Another definition."),
    ] {
        let path = sources.path().join(format!("{name}.ttl"));
        let metadata = if name == "first" {
            include_str!("../examples/specification/project.ttl")
        } else {
            ""
        };
        tokio::fs::write(&path, format!("{metadata}\n<urn:specification:subject> <http://purl.org/dc/terms/description> \"{value}\"@en .")).await?;
        let source_url = format!(
            "https://example.org/{name}/{}/ontology.ttl",
            "long_repository_path_segment_".repeat(6)
        );
        RemoteIdentity {
            inner:  RdfFile::new(
                "urn:specification:project",
                &source_url,
                path,
            )?,
            source: source_url,
        }
        .collect(&writer)
        .await?;
    }
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &oxrdf::NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    sources.close()?;
    let PublishOutcome::Published(publication) =
        Publisher::new(restored.clone())
            .render(&plan, output.path())
            .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let document = scraper::Html::parse_document(&html);
    for link in document
        .select(&scraper::Selector::parse("a.self-link[href^='#']").unwrap())
    {
        let target = link
            .value()
            .attr("href")
            .unwrap()
            .strip_prefix('#')
            .unwrap();
        assert!(
            document
                .select(&scraper::Selector::parse("[id]").unwrap())
                .any(|node| node.value().attr("id") == Some(target)),
            "self-link target must exist: {target}"
        );
    }
    let report = document
        .select(&scraper::Selector::parse("#quality-issues").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        report
            .select(&scraper::Selector::parse("pre").unwrap())
            .count(),
        0,
        "competing values must be prose, not raw RDF dumps"
    );
    let entries: Vec<_> = report
        .select(&scraper::Selector::parse(".quality-evidence").unwrap())
        .collect();
    assert!(
        entries.len() >= 2,
        "each value/source needs a readable entry"
    );
    assert!(report.text().any(|text| text.contains("Description")));
    assert!(report.text().any(|text| text.contains("English (en)")));
    assert!(
        !report.text().any(|text| text.contains("\\n")),
        "RDF escaped newlines must not leak into prose"
    );
    assert!(html.contains("&lt;script&gt; literal."));
    assert!(!html.contains("Second paragraph <script>"));
    assert!(
        report
            .select(
                &scraper::Selector::parse(".quality-evidence a[href]").unwrap()
            )
            .count() >=
            2
    );
    let mut evidence = restored.select("SELECT ?statement WHERE { GRAPH ?g { ?statement a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement> ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#subject> <urn:specification:subject> ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate> <http://purl.org/dc/terms/description> ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#object> ?value ; <http://purl.org/dc/terms/source> ?source } }".into()).await?;
    let mut evidence_count = 0;
    while evidence.next().await?.is_some() {
        evidence_count += 1;
    }
    assert_eq!(
        evidence_count, 2,
        "every value/source pair must survive as a structured statement"
    );
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("English (en)"));
    assert!(typst.contains("Description"));
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF")
    );
    if let Ok(destination) = std::env::var("GRAPHARCH_QUALITY_PREVIEW") {
        tokio::fs::create_dir_all(&destination).await?;
        let mut files = tokio::fs::read_dir(&publication.directory).await?;
        while let Some(file) = files.next_entry().await? {
            if file.file_type().await?.is_file() {
                tokio::fs::copy(
                    file.path(),
                    std::path::Path::new(&destination).join(file.file_name()),
                )
                .await?;
            }
        }
    }
    Ok(())
}
