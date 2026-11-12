//! Explicitly enabled collectors that resolve people referenced by already
//! collected contributions to their public records.
//!
//! Network access happens only for a project that declares a
//! `ga:CollectorUse` of `ga:OrcidPersonCollector` or
//! `ga:GitHubContributorCollector`. Each configured use names its API base URL
//! as `ga:source`. These collectors run after the source collectors of a
//! build, because the references they resolve are source assertions.
mod api;
mod github;
mod orcid;

use {
    super::{Collector, collect_all, kind::CollectorKind},
    crate::publication::{
        dataset::Dataset,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{GraphName, NamedNode, Quad, Term},
    std::sync::Arc,
};

const DCT: &str = "http://purl.org/dc/terms/";
const PROV: &str = "http://www.w3.org/ns/prov#";
const SCHEMA: &str = "https://schema.org/";
const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// Collect the public records of every configured person collector use.
pub(crate) async fn collect(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    concurrency: usize,
) -> Result<()> {
    collect_all(
        collectors(dataset, project).await?,
        writer,
        concurrency,
    )
    .await
}

/// Bind one collector per referenced person (ORCID) or source repository
/// (GitHub). Callers that track contribution ownership register these before
/// running them.
pub(crate) async fn collectors(
    dataset: &Dataset,
    project: &str,
) -> Result<Vec<Arc<dyn Collector>>> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project).await?,
    );
    let mut uses = view.objects(&NamedNode::new(project)?, "collector").await?;
    let mut work: Vec<Arc<dyn Collector>> = Vec::new(); // bounded: 32 uses, each capped below.
    let mut count = 0;
    while let Some(row) = uses.next().await? {
        count += 1;
        ensure!(
            count <= 32,
            "configured sources exceed 32-source budget"
        );
        let Term::NamedNode(processor) = &row["value"] else {
            bail!("configured collector must be an IRI")
        };
        let kind = CollectorKind::from_node(
            &view.node(processor, "collectorType").await?,
        )?;
        if !kind.reads_collected_people() {
            continue;
        }
        let endpoint =
            api::Endpoint::try_new(&view.text(processor, "source").await?)?;
        match kind {
            CollectorKind::OrcidPerson => {
                work.extend(
                    orcid::collectors(
                        dataset, &view, project, processor, endpoint,
                    )
                    .await?,
                )
            },
            CollectorKind::GitHubContributor => {
                work.extend(
                    github::collectors(
                        dataset, &view, project, processor, endpoint,
                    )
                    .await?,
                )
            },
            _ => unreachable!("person collector kinds are matched above"),
        }
    }
    Ok(work)
}

fn quad(subject: &NamedNode, predicate: &str, object: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(predicate),
        object,
        GraphName::DefaultGraph,
    )
}

fn integer(value: usize) -> oxrdf::Literal {
    oxrdf::Literal::new_typed_literal(
        value.to_string(),
        NamedNode::new_unchecked(XSD_INTEGER),
    )
}

/// Bound and normalize a public display string from a person API.
fn display_text(value: &str, field: &str) -> Result<Option<String>> {
    // bounded: words of one API field, itself within the response limit.
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return Ok(None);
    }
    ensure!(
        value.chars().count() <= 256 && !value.chars().any(char::is_control),
        "{field} exceeds 256 characters or contains control characters"
    );
    Ok(Some(value))
}
