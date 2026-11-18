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
<urn:repo> a schema:SoftwareSourceCode ; schema:codeRepository <https://github.com/example/ontology> ; prov:hadMember <urn:commit:tip> ; schema:version "v1.1-preview" .
<urn:commit:tip> a prov:Entity ; dct:identifier "0123456789abcdef0123456789abcdef01234567" ; prov:wasDerivedFrom <urn:repo> ; prov:wasRevisionOf <urn:commit:previous> ; prov:generatedAtTime "2026-10-30T10:20:30Z" ; dct:created "2026-10-30T09:15:00Z" ; prov:qualifiedAttribution <urn:attribution:tip>, <urn:attribution:tip-author> .
<urn:attribution:tip> prov:agent <urn:person:forge> ; prov:hadRole <urn:grapharch:git-role:committer> .
<urn:attribution:tip-author> prov:agent <urn:person:tip> ; prov:hadRole <urn:grapharch:git-role:author> .
<urn:person:forge> foaf:name "GitHub" ; schema:email "noreply@github.com" .
<urn:person:tip> foaf:name "Recent Author" ; schema:email "private@example.org" .
<urn:commit:previous> a prov:Entity ; dct:identifier "abcdef0123456789abcdef0123456789abcdef01" ; prov:generatedAtTime "2026-10-29T09:00:00Z" ; dct:created "2026-10-29T08:00:00Z" ; prov:qualifiedAttribution <urn:attribution:previous>, <urn:attribution:previous-author> .
<urn:attribution:previous> prov:agent <urn:person:forge> ; prov:hadRole <urn:grapharch:git-role:committer> .
<urn:attribution:previous-author> prov:agent <urn:person:previous> ; prov:hadRole <urn:grapharch:git-role:author> .
<urn:person:previous> foaf:name "Earlier Author" .
<urn:release> a <http://usefulinc.com/ns/doap#Version> ; prov:wasDerivedFrom <urn:commit:tip> ; dct:title "Community preview" ; schema:url <https://github.com/example/ontology/releases/tag/v1.1-preview> .
"#;

const TIP: &str = "0123456789abcdef0123456789abcdef01234567";
const PREVIOUS: &str = "abcdef0123456789abcdef0123456789abcdef01";
const OPERATOR: &str = "<https://github.com/example/ontology/actions/runs/42/attempts/1> a <http://www.w3.org/ns/prov#Activity> ; <http://www.w3.org/2000/01/rdf-schema#label> \"Publish, run 42 attempt 1\" ; <http://www.w3.org/ns/prov#used> <urn:commit:tip> ; <http://www.w3.org/ns/prov#wasAssociatedWith> <urn:github-actions>, <https://github.com/octocat> . <urn:github-actions> <http://www.w3.org/2000/01/rdf-schema#label> \"GitHub Actions\" . <https://github.com/octocat> <http://www.w3.org/2000/01/rdf-schema#label> \"octocat\" .";

fn select<'a>(
    doc: &'a scraper::Html,
    selector: &str,
) -> Vec<scraper::ElementRef<'a>> {
    doc.select(&scraper::Selector::parse(selector).unwrap())
        .collect()
}

fn rows(doc: &scraper::Html) -> Vec<(String, String)> {
    select(
        doc,
        "#document-provenance table.provenance-fields tr",
    )
    .into_iter()
    .map(|row| {
        let cell = |name| {
            row.select(&scraper::Selector::parse(name).unwrap())
                .next()
                .unwrap()
                .text()
                .collect::<String>()
        };
        (cell("th"), cell("td"))
    })
    .collect()
}

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
        definitions::iri(&format!(
            "https://ekgf.org/ontology/grapharch#{renderer}"
        ))?,
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
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
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
    let (output, html) = render("SpecificationRenderer", EVIDENCE).await?;
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
    assert!(html.contains("Recent Author") && html.contains("Earlier Author"));
    assert!(
        !section.text().collect::<String>().contains("GitHub"),
        "merge/squash committers must not replace commit authors"
    );
    assert!(
        html.contains("v1.1-preview") && html.contains("Community preview")
    );
    assert!(!html.contains("private@example.org"));
    let typst =
        tokio::fs::read_to_string(output.path().join("specification/main.typ"))
            .await?;
    assert!(
        typst.contains("Document provenance") &&
            typst.contains("Earlier Author") &&
            !typst.contains("\"GitHub\"")
    );
    assert!(typst.contains(&format!(
        "GraphArch {}",
        env!("CARGO_PKG_VERSION")
    )));
    if !env!("GRAPHARCH_BUILD_RUN_URL").is_empty() {
        for value in [
            env!("GRAPHARCH_BUILD_NUMBER"),
            env!("GRAPHARCH_BUILD_RUN_URL"),
            env!("GRAPHARCH_GENERATOR_REVISION"),
        ] {
            assert!(
                html.contains(value),
                "HTML must retain the producer identity"
            );
            assert!(
                typst.contains(value),
                "PDF source must retain the same producer identity"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn non_git_sources_report_missing_git_evidence_honestly() -> Result<()> {
    let (_, html) = render("SpecificationRenderer", "").await?;
    assert!(html.contains(
        "No Git repository revision was recorded for these sources."
    ));
    assert!(!html.contains("not recorded."));
    let doc = scraper::Html::parse_document(&html);
    assert!(
        rows(&doc)
            .iter()
            .all(|(label, _)| label != "Build operator"),
        "an unknown build operator is omitted, not described"
    );
    assert!(!html.contains("Recent Author"));
    assert!(!html.contains("https://example.org/not-a-build-source"));
    Ok(())
}

#[tokio::test]
async fn paginated_spec_links_to_its_shared_provenance_appendix() -> Result<()>
{
    let (_, html) = render("SpecificationPagesRenderer", EVIDENCE).await?;
    assert!(html.contains("id=\"document-provenance\""));
    assert!(html.contains("href=\"#document-provenance\""));
    Ok(())
}

#[tokio::test]
#[ignore = "manual HTML/PDF visual review artifact"]
async fn retain_provenance_review_document() -> Result<()> {
    let evidence = format!("{EVIDENCE}\n{OPERATOR}");
    let (output, _) = render("SpecificationRenderer", &evidence).await?;
    println!(
        "Provenance review: {}",
        output.keep().join("specification").display()
    );
    Ok(())
}

#[tokio::test]
async fn provenance_follows_quality_and_preserves_explicit_operator_and_local_state()
-> Result<()> {
    let evidence = format!("{EVIDENCE}\n<urn:repo> <http://www.w3.org/ns/prov#atLocation> <file:///example/local-clone/> .\n{OPERATOR}\n<urn:specification:subject> <http://www.w3.org/2000/01/rdf-schema#label> \"First\"@en, \"Second\"@en .");
    let (_, html) = render("SpecificationRenderer", &evidence).await?;
    let document = scraper::Html::parse_document(&html);
    let provenance = document
        .select(&scraper::Selector::parse("#document-provenance").unwrap())
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(provenance.contains("C. Document provenance"));
    let operator = rows(&document)
        .into_iter()
        .find(|(label, _)| label == "Build operator")
        .expect("recorded build operator row")
        .1;
    assert_eq!(
        operator,
        "Publish, run 42 attempt 1 (GitHub Actions, octocat)"
    );
    assert!(!select(
        &document,
        "#document-provenance td a[href='https://github.com/example/ontology/actions/runs/42/attempts/1']"
    )
    .is_empty());
    assert!(provenance.contains("may include uncommitted changes"));
    assert!(
        html.find("id=\"quality-issues\"").unwrap() <
            html.find("id=\"document-provenance\"").unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn tag_names_are_specific_to_the_repository_even_for_the_same_commit()
-> Result<()> {
    let evidence = format!("{}\n<urn:repo> <https://schema.org/version> \"source-one\" .\n<urn:second-repo> a <https://schema.org/SoftwareSourceCode> ; <https://schema.org/codeRepository> <https://github.com/example/second-source> ; <http://www.w3.org/ns/prov#hadMember> <urn:commit:tip> ; <https://schema.org/version> \"source-two\" .", EVIDENCE.replace(" ; schema:version \"v1.1-preview\"", ""));
    let (_, html) = render("SpecificationRenderer", &evidence).await?;
    let document = scraper::Html::parse_document(&html);
    let sources = document
        .select(&scraper::Selector::parse(".provenance-source").unwrap())
        .map(|s| s.text().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert!(
        sources[0].contains("source-one") && !sources[0].contains("source-two")
    );
    assert!(
        sources[1].contains("source-two") && !sources[1].contains("source-one")
    );
    Ok(())
}

#[tokio::test]
async fn provenance_accepts_sanitized_ssh_and_git_transport_origins()
-> Result<()> {
    for origin in [
        "ssh://git.example.org/ontology.git",
        "git://git.example.org/ontology.git",
    ] {
        let evidence = EVIDENCE.replace(
            "https://github.com/example/ontology>",
            &format!("{origin}>"),
        );
        let (_, html) = render("SpecificationRenderer", &evidence).await?;
        assert!(html.contains(origin));
        let doc = scraper::Html::parse_document(&html);
        assert!(
            select(
                &doc,
                "#document-provenance .provenance-source a[href*='/commit/'], \
                 #document-provenance .provenance-commits a[href*='/commit/']"
            )
            .is_empty(),
            "only known forges get commit links"
        );
        assert!(html.contains(&format!("<code>{TIP}</code>")));
    }
    Ok(())
}

#[tokio::test]
async fn provenance_is_one_table_with_linked_short_commits_and_no_duplicates()
-> Result<()> {
    let (output, html) = render("SpecificationRenderer", EVIDENCE).await?;
    let doc = scraper::Html::parse_document(&html);
    let labels = rows(&doc)
        .into_iter()
        .map(|(label, _)| label)
        .collect::<Vec<_>>();
    let expected_head = if env!("GRAPHARCH_GENERATOR_REVISION").is_empty() {
        vec!["Generator"]
    } else {
        vec!["Generator", "Generator commit"]
    };
    assert_eq!(labels[..expected_head.len()], expected_head[..]);
    assert_eq!(labels[expected_head.len()..], [
        "Repository",
        "Source commit",
        "Source state",
        "Git tags",
        "Release"
    ]);
    assert_eq!(
        select(&doc, "#document-provenance table").len(),
        1
    );
    let section = select(&doc, "#document-provenance")[0]
        .text()
        .collect::<String>();
    assert!(!section.contains("Source repository"));
    assert!(!section.contains("Grapharch") && !section.contains("Graparch"));
    assert!(section.contains(&format!(
        "GraphArch {}",
        env!("CARGO_PKG_VERSION")
    )));
    for (sha, link) in [(TIP, "Source commit"), (PREVIOUS, "Recent commits")] {
        let anchors = select(
            &doc,
            &format!(
                "#document-provenance a[href='https://github.com/example/ontology/commit/{sha}']"
            ),
        );
        assert!(!anchors.is_empty(), "{link} must link {sha}");
        assert_eq!(
            section.matches(&sha[..12]).count(),
            1,
            "{sha} must be shown exactly once"
        );
        assert!(
            !section.contains(sha),
            "full SHA is not shown as text"
        );
    }
    let tip = select(
        &doc,
        &format!("#document-provenance td a[title='{TIP}'] code"),
    );
    assert_eq!(tip[0].text().collect::<String>(), &TIP[..12]);
    let revision = env!("GRAPHARCH_GENERATOR_REVISION");
    if !revision.is_empty() {
        assert_eq!(
            select(
                &doc,
                &format!(
                    "#document-provenance a[href='https://github.com/EKGF/grapharch/commit/{revision}']"
                )
            )
            .len(),
            1
        );
        assert_eq!(section.matches(&revision[..12]).count(), 1);
    }
    assert_eq!(
        section
            .matches("https://github.com/example/ontology")
            .count(),
        1
    );
    let typst =
        tokio::fs::read_to_string(output.path().join("specification/main.typ"))
            .await?;
    assert!(typst.contains(&format!(
        "#link(\"https://github.com/example/ontology/commit/{PREVIOUS}\")[#raw(\"{}\")]",
        &PREVIOUS[..12]
    )));
    assert!(typst.contains("#ga-table(columns: (auto, 1fr)"));
    assert!(!typst.contains("Source repository"));
    Ok(())
}
