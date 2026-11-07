//! Publication-neutral source subject bindings used by native diagram links.
use {
    crate::publication::{dataset::Dataset, term, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};
pub(super) struct Page {
    pub id:    NamedNode,
    pub route: String,
}
pub(super) async fn read(
    dataset: &Dataset,
    view: &RevisionView,
) -> Result<BTreeMap<NamedNode, Page>> {
    let facts = view.pattern(
        &format!(
            "?subject a <{}UseCase> ; {} ?path",
            super::model::UC,
            term("sourcePath")
        ),
        0,
    );
    let pages = view.pattern(
        &format!(
            "?page a {} ; {} ?path ; {} ?route",
            term("Page"),
            term("sourcePath"),
            term("route")
        ),
        1,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject ?page ?route WHERE {{ {facts} OPTIONAL \
             {{ {pages} }} }} ORDER BY ?subject LIMIT 257"
        ))
        .await?;
    let mut result = BTreeMap::new(); // bounded: at most 256 explicitly collected authored use cases.
    while let Some(row) = rows.next().await? {
        ensure!(
            result.len() < 256,
            "authored use-case page bindings exceed 256 entries"
        );
        let Term::NamedNode(subject) = &row["subject"] else {
            anyhow::bail!("use-case subject must be named")
        };
        let (Some(Term::NamedNode(page)), Some(Term::Literal(route))) =
            (row.get("page"), row.get("route"))
        else {
            anyhow::bail!(
                "authored use case has no collected page/route: {subject}"
            )
        };
        ensure!(
            result
                .insert(subject.clone(), Page {
                    id:    page.clone(),
                    route: route.value().into(),
                })
                .is_none(),
            "ambiguous authored use-case page: {subject}"
        );
    }
    Ok(result)
}
