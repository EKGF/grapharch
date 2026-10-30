use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, showcases},
    std::path::Path,
};

#[tokio::test]
async fn dedicated_skos_and_use_case_gallery_examples_are_executable()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let examples = root.path().join("examples");
    for name in [
        "skos/project.ttl",
        "skos/sources/taxonomy.ttl",
        "skos/sources/reference-data.ttl",
        "use-cases/project.ttl",
        "use-cases/use-cases.ttl",
    ] {
        let destination = examples.join(name);
        tokio::fs::create_dir_all(destination.parent().unwrap()).await?;
        tokio::fs::copy(Path::new("examples").join(name), destination).await?;
    }
    let registry = tokio::fs::read_to_string("examples/showcases.ttl").await?;
    let registry = registry
        .lines()
        .map(|line| {
            if line.starts_with("<urn:grapharch:docs> ga:showcase ") {
                "<urn:grapharch:docs> ga:showcase ex:skos, ex:use-cases ."
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    tokio::fs::write(examples.join("showcases.ttl"), registry).await?;
    let output = tempfile::tempdir()?;
    let report = showcases::verify_gallery(
        Dataset::memory()?,
        &examples,
        "urn:grapharch:docs",
        output.path(),
    )
    .await?;
    report.ensure_success()?;
    assert_eq!(report.cases.len(), 2);
    for case in &report.cases {
        assert!(case.dataset_archive.as_ref().unwrap().is_file());
    }
    let skos = tokio::fs::read_to_string(
        output
            .path()
            .join("artifacts/skos/specification/index.html"),
    )
    .await?;
    for text in [
        "Business data taxonomy",
        "Explore the 1 vocabulary",
        "Concepts",
        "Collections",
        "Offering",
        "skos:memberList",
    ] {
        assert!(skos.contains(text), "missing {text}");
    }
    let cases = tokio::fs::read_to_string(
        output.path().join("artifacts/use-cases/website/index.html"),
    )
    .await?;
    for text in [
        "Customer insight",
        "Reference information",
        "Review analyst",
        "Verified identity",
        "UseCaseStereoTypeBusinessCapabilityArea",
        "Ownership parent",
        "Reused in",
    ] {
        assert!(cases.contains(text), "missing {text}");
    }
    assert!(cases.contains("<img"));
    Ok(())
}

#[tokio::test]
async fn semantic_showcases_render_from_restored_kg_after_deleting_sources()
-> Result<()> {
    use grapharch::publication::render::Publisher;
    for kind in ["skos", "use-cases"] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let files: &[&str] = if kind == "skos" {
            &[
                "project.ttl",
                "sources/taxonomy.ttl",
                "sources/reference-data.ttl",
            ]
        } else {
            &["project.ttl", "use-cases.ttl"]
        };
        for file in files {
            let destination = source.join(file);
            tokio::fs::create_dir_all(destination.parent().unwrap()).await?;
            tokio::fs::copy(
                Path::new("examples").join(kind).join(file),
                destination,
            )
            .await?;
        }
        let (dataset, plans) = if kind == "skos" {
            showcases::skos(&source).await?
        } else {
            showcases::use_cases(&source).await?
        };
        let archive = temp.path().join("documentation.nq");
        dataset.export(archive.clone()).await?;
        tokio::fs::remove_dir_all(source).await?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let output = temp.path().join("published");
        let publications =
            showcases::publish(&Publisher::new(restored), &plans, &output)
                .await?;
        assert_eq!(
            publications.len(),
            if kind == "skos" { 1 } else { 2 }
        );
        let pdf = output.join(
            if kind == "skos" {
                "specification/document.pdf"
            } else {
                "review/document.pdf"
            },
        );
        assert!(tokio::fs::read(pdf).await?.starts_with(b"%PDF-"));
        if kind == "use-cases" {
            let mut assets =
                tokio::fs::read_dir(output.join("website/assets")).await?;
            let mut count = 0;
            while let Some(asset) = assets.next_entry().await? {
                if asset.path().extension().is_some_and(|e| e == "svg") {
                    let text = tokio::fs::read_to_string(asset.path()).await?;
                    let svg = roxmltree::Document::parse(&text)?;
                    assert_eq!(
                        svg.descendants()
                            .filter(|e| e.has_tag_name("rect"))
                            .count(),
                        1
                    );
                    assert!(!text.contains("<script"));
                    count += 1;
                }
            }
            assert_eq!(count, 6);
            let html =
                tokio::fs::read_to_string(output.join("website/index.html"))
                    .await?;
            let doc = scraper::Html::parse_document(&html);
            let ids = doc
                .select(&scraper::Selector::parse("[id]").unwrap())
                .filter_map(|e| e.value().id())
                .collect::<std::collections::HashSet<_>>();
            for link in
                doc.select(&scraper::Selector::parse("a[href^='#']").unwrap())
            {
                assert!(ids.contains(
                    link.value().attr("href").unwrap().trim_start_matches('#')
                ));
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn use_case_ownership_cycles_and_dangling_links_fail_before_publication()
-> Result<()> {
    for assertion in [
        "ex:CustomerInsight uc:isPartOf ex:CheckIdentity .",
        "ex:CustomerInsight uc:isPartOf ex:Missing .",
        "ex:CheckIdentity uc:isPartOf ex:CustomerOverview .",
    ] {
        let temp = tempfile::tempdir()?;
        tokio::fs::copy(
            "examples/use-cases/project.ttl",
            temp.path().join("project.ttl"),
        )
        .await?;
        let source =
            tokio::fs::read_to_string("examples/use-cases/use-cases.ttl")
                .await?;
        tokio::fs::write(
            temp.path().join("use-cases.ttl"),
            format!("{source}\n{assertion}"),
        )
        .await?;
        let error = showcases::use_cases(temp.path())
            .await
            .err()
            .expect("invalid ownership must fail");
        let message = format!("{error:#}");
        assert!(
            message.contains("cycle") ||
                message.contains("dangling") ||
                message.contains("multiple ownership parents"),
            "{message}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn removed_use_cases_retire_generated_fragments_and_assets() -> Result<()>
{
    use {
        grapharch::publication::{
            collect::{Collector, RdfFile},
            document::{Documentor, UseCaseDocumentor},
            term,
            writer::GraphWriter,
        },
        oxrdf::{GraphName, NamedNode, Quad},
    };
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("cases.ttl");
    let parent="@prefix uc:<https://ekgf.org/ontology/use-case#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . <urn:parent> a uc:UseCase;rdfs:label \"A & B <review>\"@en;rdfs:comment \"A parent.\"@en .";
    tokio::fs::write(
        &path,
        format!(
            "{parent}\n<urn:child> a uc:UseCase;rdfs:label \
             \"Child\"@en;rdfs:comment \"A child.\"@en;uc:isPartOf \
             <urn:parent> ."
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let collector = RdfFile::new(
        "urn:test:cases",
        "urn:test:collector",
        path.clone(),
    )?;
    collector.collect(&writer).await?;
    let doc = UseCaseDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:test:cases").await?,
        "urn:test:cases",
    )?;
    doc.document(&writer).await?;
    let before = dataset.snapshot().await?;
    let child = NamedNode::new("urn:child")?;
    let subject = Quad::new(
        doc.fragment_id(&child),
        term("subject"),
        child,
        GraphName::DefaultGraph,
    );
    assert!(dataset.contains(&before, subject.clone()).await?);
    tokio::fs::write(path, parent).await?;
    collector.collect(&writer).await?;
    UseCaseDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:test:cases").await?,
        "urn:test:cases",
    )?
    .document(&writer)
    .await?;
    let after = dataset.snapshot().await?;
    assert!(!dataset.contains(&after, subject.clone()).await?);
    assert!(dataset.contains(&before, subject).await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn empty_related_resource_labels_fail_instead_of_publishing_empty_cells()
-> Result<()> {
    let temp = tempfile::tempdir()?;
    tokio::fs::copy(
        "examples/use-cases/project.ttl",
        temp.path().join("project.ttl"),
    )
    .await?;
    let source = tokio::fs::read_to_string("examples/use-cases/use-cases.ttl")
        .await?
        .replace("\"Review analyst\"@en", "\"\"@en");
    tokio::fs::write(temp.path().join("use-cases.ttl"), source).await?;
    let error = showcases::use_cases(temp.path())
        .await
        .err()
        .expect("empty labels must fail");
    assert!(format!("{error:#}").contains("invalid related-resource label"));
    Ok(())
}
