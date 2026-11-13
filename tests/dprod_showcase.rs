#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;
#[path = "support/mod.rs"]
mod specification_assertions;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::Publisher,
        revision_build::TargetSelection,
        showcases,
    },
    std::path::Path,
};

#[tokio::test]
async fn dprod_native_rdf_and_markdown_publish_from_a_restored_graph()
-> Result<()> {
    let fixture =
        revision_fixture::RevisionFixture::new(Path::new("examples/dprod"))?;
    let (dataset, plans) = fixture.collect(TargetSelection::All).await?;
    let mut shapes = dataset.select("SELECT (COUNT(DISTINCT ?shape) AS ?count) WHERE { GRAPH ?g { ?shape a ?kind } VALUES ?kind { <http://www.w3.org/ns/shacl#NodeShape> <http://www.w3.org/ns/shacl#PropertyShape> } }".into()).await?;
    let row = shapes.next().await?.unwrap();
    let oxrdf::Term::Literal(count) = &row["count"] else {
        panic!("count required")
    };
    assert_eq!(count.value(), "34");
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    fixture.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications = showcases::publish(
        &Publisher::new(restored)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans,
        output.path(),
    )
    .await?;
    assert_eq!(publications.len(), 3);
    let revisions: std::collections::HashSet<_> = publications
        .iter()
        .map(|publication| publication.manifest.input_revision.as_str())
        .collect();
    assert_eq!(
        revisions.len(),
        1,
        "all publications must use the same assembled revision"
    );
    let profiles: std::collections::HashSet<_> = publications
        .iter()
        .map(|publication| publication.manifest.profile.as_str())
        .collect();
    assert_eq!(
        profiles.len(),
        3,
        "each publication has an independent arrangement"
    );
    let site =
        std::fs::read_to_string(output.path().join("website/index.html"))?;
    let guide = std::fs::read_to_string(
        output
            .path()
            .join("website/guides/getting-started/index.html"),
    )?;
    assert!(site.contains("Make data products understandable"));
    assert!(guide.contains("Start with a product people can identify"));
    assert!(!site.contains("SHACL property shapes"));
    assert!(!site.contains("DataProduct-dataProductLifecycleStatus"));
    let pdf = std::fs::read(only_pdf(&output.path().join("pdf")))?;
    assert!(pdf.starts_with(b"%PDF-"));
    for publication in &publications {
        assert!(publication.manifest.diagnostics.is_empty());
    }
    let html = std::fs::read_to_string(
        output.path().join("specification/index.html"),
    )?;
    for expected in [
        "DPROD",
        "DataProduct",
        "A rational, managed, and governed collection of data",
        "InformationSensitivityClassification",
        "dataProductOwner",
        "DataProduct-dataProductLifecycleStatus",
        "single skos:Concept value",
        "Dataset schema example",
        "sh:maxCount",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    specification_assertions::assert_specification_scripts(&html);
    assert!(!html.contains("Make data products understandable"));
    assert!(!html.contains("Start with a product people can identify"));
    assert!(
        std::fs::read(only_pdf(&output.path().join("specification")))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn dprod_fixture_matches_its_pinned_native_sources() -> Result<()> {
    let verified = grapharch::publication::showcases::verify_copied_sources(
        Path::new("examples/dprod/showcase.ttl"),
    )
    .await?;
    assert_eq!(verified.len(), 4);
    Ok(())
}

/// The one PDF edition published in `directory`.
#[allow(dead_code)]
fn only_pdf(directory: &std::path::Path) -> std::path::PathBuf {
    let mut pdfs = std::fs::read_dir(directory)
        .expect("PDF directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pdf"))
        .collect::<Vec<_>>();
    assert_eq!(
        pdfs.len(),
        1,
        "one PDF edition in {}",
        directory.display()
    );
    pdfs.remove(0)
}
