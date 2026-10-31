#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
};

const EVIDENCE: &str = r#"
@prefix prov: <http://www.w3.org/ns/prov#> .
@prefix schema: <https://schema.org/> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
<urn:repo> a schema:SoftwareSourceCode ; schema:codeRepository <https://github.com/example/ontology> ; prov:hadMember <urn:commit:tip> .
<urn:commit:tip> a prov:Entity ; dct:identifier "0123456789abcdef0123456789abcdef01234567" ; prov:wasDerivedFrom <urn:repo> ; prov:wasRevisionOf <urn:commit:previous> ; prov:generatedAtTime "2026-10-30T10:20:30Z" ; prov:qualifiedAttribution <urn:attribution:tip> ; schema:version "v1.1-preview" .
<urn:attribution:tip> prov:agent <urn:person:tip> ; prov:hadRole <urn:grapharch:git-role:committer> .
<urn:person:tip> foaf:name "Recent Committer" ; schema:email "private@example.org" .
<urn:commit:previous> a prov:Entity ; dct:identifier "abcdef0123456789abcdef0123456789abcdef01" ; prov:generatedAtTime "2026-10-29T09:00:00Z" ; prov:qualifiedAttribution <urn:attribution:previous> .
<urn:attribution:previous> prov:agent <urn:person:previous> ; prov:hadRole <urn:grapharch:git-role:committer> .
<urn:person:previous> foaf:name "Earlier Committer" .
<urn:release> a <http://usefulinc.com/ns/doap#Version> ; prov:wasDerivedFrom <urn:commit:tip> ; dct:title "Community preview" ; schema:url <https://github.com/example/ontology/releases/tag/v1.1-preview> .
"#;

async fn render(
    renderer: &str,
    evidence: &str,
) -> Result<(tempfile::TempDir, String)> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#renderer",
        definitions::literal(renderer),
    )?;
    tokio::fs::write(&path, format!("{fixture}\n{evidence}\n<urn:documented-repo> a <https://schema.org/SoftwareSourceCode> ; <https://schema.org/codeRepository> <https://example.org/not-a-build-source> ; <http://www.w3.org/ns/prov#hadMember> <urn:not-a-build-commit> .")).await?;
    let (dataset, _) = showcases::specification(&path).await?;
    if !evidence.is_empty() {
        let attempt = dataset
            .begin(
                "urn:specification:project",
                "urn:grapharch:git-provenance",
                "https://github.com/example/ontology",
                "fixture-commit",
            )
            .await?;
        let quads = oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
            .for_slice(evidence.as_bytes())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        attempt.push(quads).await?;
        attempt.complete().await?;
    }
    let revision =
        grapharch::publication::assembly::Assembler::new(dataset.clone())
            .assemble("urn:specification:project")
            .await?;
    let (writer, worker) = grapharch::publication::writer::GraphWriter::spawn(
        dataset.clone(),
        8,
        1024 * 1024,
    )?;
    let plan = grapharch::publication::plan::TargetPlanner::new(
        dataset.clone(),
        revision,
    )
    .resolve(
        "urn:specification:project",
        &oxrdf::NamedNode::new("urn:specification:target")?,
        &writer,
    )
    .await?;
    drop(writer);
    worker.await??;
    let export = tempfile::tempdir()?;
    dataset.export(export.path().join("source.nq")).await?;
    source.close()?;
    let restored = grapharch::publication::dataset::Dataset::memory()?;
    restored.import(export.path().join("source.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    Ok((output, html))
}

#[tokio::test]
async fn provenance_is_a_linked_closing_appendix_from_restored_kg() -> Result<()>
{
    let (output, html) = render("specification", EVIDENCE).await?;
    let doc = scraper::Html::parse_document(&html);
    let section = doc
        .select(
            &scraper::Selector::parse("section#document-provenance").unwrap(),
        )
        .next()
        .expect("closing provenance appendix");
    assert!(
        section
            .select(
                &scraper::Selector::parse(
                    "h2 a.self-link[href='#document-provenance']"
                )
                .unwrap()
            )
            .next()
            .is_some()
    );
    assert!(
        doc.select(
            &scraper::Selector::parse("nav#toc a[href='#document-provenance']")
                .unwrap()
        )
        .next()
        .is_some()
    );
    assert!(
        section
            .text()
            .collect::<String>()
            .contains("B. Document provenance")
    );
    assert!(
        html.contains("Recent Committer") && html.contains("Earlier Committer")
    );
    assert!(html.contains("0123456789abcdef0123456789abcdef01234567"));
    assert!(
        html.contains("v1.1-preview") && html.contains("Community preview")
    );
    assert!(!html.contains("private@example.org"));
    let typst =
        tokio::fs::read_to_string(output.path().join("specification/main.typ"))
            .await?;
    assert!(
        typst.contains("Document provenance") &&
            typst.contains("Earlier Committer")
    );
    assert!(typst.contains(env!("CARGO_PKG_VERSION")));
    Ok(())
}

#[tokio::test]
async fn non_git_sources_report_missing_git_evidence_honestly() -> Result<()> {
    let (_, html) = render("specification", "").await?;
    assert!(html.contains(
        "No Git repository revision was recorded for these sources."
    ));
    assert!(html.contains("Build operator was not recorded."));
    assert!(!html.contains("Recent Committer"));
    assert!(!html.contains("https://example.org/not-a-build-source"));
    Ok(())
}

#[tokio::test]
async fn paginated_spec_links_to_its_shared_provenance_appendix() -> Result<()>
{
    let (_, html) = render("specification-pages", EVIDENCE).await?;
    assert!(html.contains("id=\"document-provenance\""));
    assert!(html.contains("href=\"#document-provenance\""));
    Ok(())
}

#[tokio::test]
#[ignore = "manual HTML/PDF visual review artifact"]
async fn retain_provenance_review_document() -> Result<()> {
    let (output, _) = render("specification", EVIDENCE).await?;
    println!(
        "Provenance review: {}",
        output.keep().join("specification").display()
    );
    Ok(())
}

#[tokio::test]
async fn provenance_follows_quality_and_preserves_explicit_operator_and_local_state()
-> Result<()> {
    let evidence = format!("{EVIDENCE}\n<urn:repo> <http://www.w3.org/ns/prov#atLocation> <file:///example/local-clone/> .\n<urn:specification:target> <http://www.w3.org/ns/prov#wasGeneratedBy> <urn:build> . <urn:build> <http://www.w3.org/ns/prov#wasAssociatedWith> <urn:operator> . <urn:operator> <http://www.w3.org/2000/01/rdf-schema#label> \"Documentation CI\" .\n<urn:specification:subject> <http://www.w3.org/2000/01/rdf-schema#label> \"First\"@en, \"Second\"@en .");
    let (_, html) = render("specification", &evidence).await?;
    let document = scraper::Html::parse_document(&html);
    let provenance = document
        .select(&scraper::Selector::parse("#document-provenance").unwrap())
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(provenance.contains("C. Document provenance"));
    assert!(provenance.contains("Documentation CI"));
    assert!(provenance.contains("uncommitted changes"));
    assert!(
        html.find("id=\"quality-issues\"").unwrap() <
            html.find("id=\"document-provenance\"").unwrap()
    );
    Ok(())
}
