//! Local feature-catalogue checks; no remote showcase sources are contacted.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag},
    serde::Deserialize,
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
    },
};

#[derive(Deserialize)]
struct Identity {
    subject: String,
}
#[derive(Deserialize)]
struct Page {
    title:       String,
    parent:      Option<String>,
    permalink:   String,
    #[serde(default)]
    nav_exclude: bool,
    grapharch:   Identity,
}
fn pages(root: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            pages(&entry.path(), result)?;
        } else if entry.path().extension().is_some_and(|v| v == "md") {
            result.push(entry.path());
        }
    }
    ensure!(
        result.len() <= 64,
        "feature documentation exceeds its bounded test corpus"
    );
    Ok(())
}
fn read(path: &Path) -> Result<(Page, String)> {
    let text = std::fs::read_to_string(path)?;
    let body = text.strip_prefix("---\n").ok_or_else(|| {
        anyhow::anyhow!("missing frontmatter: {}", path.display())
    })?;
    let (frontmatter, content) = body
        .split_once("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("unterminated frontmatter"))?;
    Ok((serde_yaml::from_str(frontmatter)?, content.into()))
}
#[test]
fn features_form_a_current_product_catalogue_with_resolvable_navigation()
-> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new(); // bounded: the repository's <=64 feature pages.
    pages(&root.join("docs/features"), &mut paths)?;
    let mut titles = BTreeSet::new();
    let mut routes = BTreeSet::new();
    let mut subjects = BTreeSet::new();
    let mut metadata = BTreeMap::new();
    for path in &paths {
        let (page, body) = read(path)?;
        ensure!(
            titles.insert(page.title.clone()),
            "duplicate feature title {}",
            page.title
        );
        ensure!(
            routes.insert(page.permalink.clone()),
            "duplicate feature route {}",
            page.permalink
        );
        ensure!(
            subjects.insert(page.grapharch.subject.clone()),
            "duplicate logical identity"
        );
        oxrdf::NamedNode::new(&page.grapharch.subject)?;
        ensure!(
            !body.trim().is_empty(),
            "empty feature page {}",
            path.display()
        );
        metadata.insert(path, page);
    }
    let children: BTreeSet<_> = metadata
        .values()
        .filter(|page| {
            !page.nav_exclude && page.parent.as_deref() == Some("Features")
        })
        .map(|page| page.title.as_str())
        .collect();
    assert_eq!(
        children,
        BTreeSet::from([
            "Collect existing sources",
            "Reusable content and hints",
            "Queryable documentation graph",
            "Websites and PDFs",
            "Quality review",
            "Local builds and CI",
            "Extend GraphArch"
        ])
    );
    for page in metadata.values().filter(|page| !page.nav_exclude) {
        if let Some(parent) = &page.parent {
            ensure!(
                titles.contains(parent),
                "missing parent {parent} for {}",
                page.title
            );
        }
    }
    for path in paths {
        let (_, body) = read(&path)?;
        for event in Parser::new(&body) {
            let Event::Start(Tag::Link { dest_url, .. }) = event else {
                continue
            };
            if url::Url::parse(&dest_url).is_ok() || dest_url.starts_with('#') {
                continue;
            }
            let relative = dest_url.split('#').next().unwrap();
            ensure!(
                path.parent().unwrap().join(relative).exists(),
                "unresolved feature link in {}: {relative}",
                path.display()
            );
        }
    }
    for obsolete in [
        "general/README.md",
        "formats/README.md",
        "sources/files/csv.md",
        "sources/files/json.md",
        "sources/files/xml.md",
        "sources/files/yaml.md",
        "sources/files/toml.md",
        "sources/endpoints/sql.md",
        "sources/endpoints/graphql.md",
        "sources/endpoints/cypher.md",
    ] {
        ensure!(
            !root.join("docs/features").join(obsolete).exists(),
            "obsolete capability placeholder remains: {obsolete}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn explicit_rdf_file_collection_accepts_self_contained_json_ld()
-> Result<()> {
    use grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        writer::GraphWriter,
    };
    let source = tempfile::tempdir()?;
    let path = source.path().join("example.jsonld");
    tokio::fs::write(&path,r#"{"@context":{"ex":"https://example.org/"},"@id":"ex:term","ex:label":"Collected JSON-LD"}"#).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        "urn:features:project",
        "urn:features:jsonld",
        path,
    )?
    .collect(&writer)
    .await?;
    drop(writer);
    worker.await??;
    let revision = dataset.snapshot_for("urn:features:project").await?;
    assert!(
        dataset
            .contains(
                &revision,
                oxrdf::Quad::new(
                    oxrdf::NamedNode::new("https://example.org/term")?,
                    oxrdf::NamedNode::new("https://example.org/label")?,
                    oxrdf::Literal::new_simple_literal("Collected JSON-LD"),
                    oxrdf::GraphName::DefaultGraph
                )
            )
            .await?
    );
    Ok(())
}
