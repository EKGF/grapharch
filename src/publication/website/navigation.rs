//! Resolve collected source-path menu hints to real page identities before
//! planning.
use {
    crate::publication::{
        dataset::{Dataset, Stage},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Quad, Term},
    std::collections::BTreeMap,
};

pub(super) async fn bind(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
) -> Result<()> {
    let collected = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), collected.clone());
    let mut entries = dataset
        .select(format!(
            "SELECT DISTINCT ?entry ?path WHERE {{ {} }} ORDER BY ?entry",
            view.pattern(
                &format!(
                    "?entry a {} ; {} ?path",
                    term("NavigationEntry"),
                    term("sourcePath")
                ),
                0
            )
        ))
        .await?;
    let mut pending = Vec::new(); // bounded: three regions of 512 entries.
    while let Some(row) = entries.next().await? {
        ensure!(
            pending.len() < 1536,
            "navigation bindings exceed 1536 entries"
        );
        let (Term::NamedNode(entry), Term::Literal(path)) =
            (&row["entry"], &row["path"])
        else {
            anyhow::bail!(
                "navigation source path must be a literal on an identified \
                 entry"
            )
        };
        ensure!(
            view.optional(entry, "page").await?.is_none() &&
                view.optional(entry, "url").await?.is_none(),
            "navigation entry cannot combine sourcePath with page or URL"
        );
        pending.push((entry.clone(), path.value().to_owned()));
    }
    if pending.is_empty() {
        return Ok(());
    }
    let pages_view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project).await?,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?page ?path WHERE {{ {} }}",
            pages_view.pattern(
                &format!(
                    "?page a {} ; {} ?path",
                    term("Page"),
                    term("sourcePath")
                ),
                0
            )
        ))
        .await?;
    let mut pages = BTreeMap::new(); // bounded: website page discovery limit.
    while let Some(row) = rows.next().await? {
        ensure!(
            pages.len() < 1024,
            "navigation page index exceeds 1024 pages"
        );
        let (Term::NamedNode(page), Term::Literal(path)) =
            (&row["page"], &row["path"])
        else {
            anyhow::bail!("invalid page identity or source path")
        };
        ensure!(
            pages
                .insert(path.value().to_owned(), page.clone())
                .is_none(),
            "ambiguous navigation source path {}",
            path.value()
        );
    }
    let mut stream = writer
        .begin_stage(
            project,
            "urn:grapharch:website-navigation-bindings",
            project,
            collected.as_str(),
            Stage::Document,
        )
        .await?;
    for (entry, path) in pending {
        let page = pages.get(&path).ok_or_else(|| {
            anyhow::anyhow!(
                "navigation source path has no collected page: {path}"
            )
        })?;
        stream
            .write(Quad::new(
                entry,
                term("page"),
                page.clone(),
                GraphName::DefaultGraph,
            ))
            .await?;
    }
    stream.complete().await
}
