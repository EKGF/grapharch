#[path = "support/mod.rs"]
mod specification_assertions;

use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    std::path::Path,
};

#[tokio::test]
async fn dprod_native_rdf_and_markdown_publish_from_a_restored_graph()
-> Result<()> {
    let source = tempfile::tempdir()?;
    copy_tree(Path::new("examples/dprod"), source.path())?;
    let (dataset, plans) = showcases::dprod(source.path()).await?;
    let mut shapes = dataset.select("SELECT (COUNT(DISTINCT ?shape) AS ?count) WHERE { GRAPH ?g { ?shape a ?kind } VALUES ?kind { <http://www.w3.org/ns/shacl#NodeShape> <http://www.w3.org/ns/shacl#PropertyShape> } }".into()).await?;
    let row = shapes.next().await?.unwrap();
    let oxrdf::Term::Literal(count) = &row["count"] else {
        panic!("count required")
    };
    assert_eq!(count.value(), "34");
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output.path())
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
    let pdf = std::fs::read(output.path().join("pdf/document.pdf"))?;
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
        std::fs::read(output.path().join("specification/document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[test]
fn dprod_fixture_matches_its_pinned_native_sources() -> Result<()> {
    use sha2::{Digest, Sha256};
    let metadata: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../examples/dprod/reference.yaml"))?;
    for (file, entry) in metadata["sources"].as_mapping().unwrap() {
        let bytes = std::fs::read(
            Path::new("examples/dprod/sources").join(file.as_str().unwrap()),
        )?;
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            entry["sha256"].as_str().unwrap()
        );
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(())
}
