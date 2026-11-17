//! Acceptance of the GraphArch OMG Commons edition against OMG's formal
//! Commons 1.3 specification (formal/26-06-05).
//!
//! The live test publishes the edition from OMG's sources, fetches OMG's PDF
//! (pinned by SHA-256), compares structure, typography and terms, renders
//! corresponding pages side by side, and writes everything to
//! `output/commons-acceptance/`.
#[path = "support/commons_acceptance/mod.rs"]
mod commons_acceptance;
#[path = "support/pdf_pages/mod.rs"]
mod pdf_pages;

use {
    anyhow::Result,
    commons_acceptance::{
        Acceptance,
        AcceptanceReport,
        Criterion,
        Edition,
        OmgReference,
        PageAnchor,
        PagePair,
        Side,
    },
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    std::path::{Path, PathBuf},
};

/// Criteria the edition does not meet yet, each with the work that meets it.
/// A criterion leaves this list in the change that meets it.
const OPEN: &[(Criterion, &str)] = &[
    (
        Criterion::CoverTypography,
        "owner decision: rows at 11 pt keep the Edition row on one line",
    ),
    (
        Criterion::PageSize,
        "owner decision: US Letter, as the OMG LaTeX template",
    ),
    (
        Criterion::OntologyChapters,
        "owner decision: chapter order, \"Ontology:\" titles, About chapter",
    ),
    (
        Criterion::FigureCaptions,
        "follows the ontology chapter order",
    ),
    (
        Criterion::TableCaptions,
        "owner decision: \"Commons\" in ontology titles; OMG's 6-1/7-1 hyphen \
         vs 8.1 dot; About chapter metadata table",
    ),
    (
        Criterion::FrontMatterPages,
        "owner decision: front matter length",
    ),
    (
        Criterion::PageCount,
        "136 vs 166 pages: OMG table rows carry more vertical space",
    ),
    (
        Criterion::Terms,
        "harness: Name cells wrap long local names at different points",
    ),
];

#[tokio::test]
#[ignore = "live OMG source acceptance; requires network"]
async fn commons_edition_meets_the_omg_formal_specification() -> Result<()> {
    let root = Path::new("output/commons-acceptance");
    let ours = Edition::read(
        Side::GraphArch,
        tokio::fs::read(publish(root).await?).await?,
    )?;
    let omg = Edition::read(
        Side::Omg,
        OmgReference::bytes(Path::new("output/cache/omg")).await?,
    )?;
    let findings = Acceptance { omg: &omg, ours: &ours }.findings()?;
    let pages = root.join("pages");
    let pairs = PageAnchor::ALL
        .iter()
        .enumerate()
        .map(|(index, anchor)| {
            PagePair::render(*anchor, &omg, &ours, &pages, index)
        })
        .collect::<Result<Vec<_>>>()?;
    let report = AcceptanceReport {
        findings: &findings,
        pairs:    &pairs,
        open:     OPEN,
    };
    report.write(&root.join("report.md"))?;
    eprintln!("{}", report.markdown());
    let unexpected: Vec<_> = findings
        .iter()
        .filter(|finding| {
            finding.met ==
                OPEN.iter()
                    .any(|(criterion, _)| *criterion == finding.criterion)
        })
        .map(|finding| {
            if finding.met {
                format!(
                    "{} is met; remove it from OPEN",
                    finding.criterion
                )
            } else {
                format!("{} is not met: {:?}", finding.criterion, finding)
            }
        })
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:#?}");
    Ok(())
}

/// Publish the Commons showcase from a restored KG into `root`; returns its
/// PDF edition.
async fn publish(root: &Path) -> Result<PathBuf> {
    let edition = root.join("edition");
    tokio::fs::create_dir_all(&edition).await?;
    let (dataset, plans) = showcases::family(
        Path::new("examples/omg-commons/showcase.ttl"),
        Path::new("output/cache/git"),
    )
    .await?;
    let archive = edition.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    showcases::publish(
        &Publisher::new(restored)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans,
        &edition,
    )
    .await?;
    let mut pdfs = std::fs::read_dir(edition.join("pdf"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    pdfs.retain(|path| {
        path.extension().is_some_and(|extension| extension == "pdf")
    });
    anyhow::ensure!(
        pdfs.len() == 1,
        "one PDF edition expected, found {pdfs:?}"
    );
    Ok(pdfs.remove(0))
}
