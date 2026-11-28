//! The newest Git change of each page source, recorded at collection.
use {
    super::{Catalog, Plan},
    crate::publication::term,
    anyhow::{Result, ensure},
    oxrdf::Term,
};

/// Read `ga:PageRevision` dates into the catalog, keyed by source path.
pub(super) async fn read(plan: &Plan, catalog: &mut Catalog) -> Result<()> {
    let pattern = plan.source.pattern(
        &format!(
            "?revision a {} ; {} ?path ; <http://purl.org/dc/terms/modified> \
             ?date",
            term("PageRevision"),
            term("sourcePath")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?path ?date WHERE {{ {pattern} }}"
        ))
        .await?;
    while let Some(row) = rows.next().await? {
        let (Term::Literal(path), Term::Literal(date)) =
            (&row["path"], &row["date"])
        else {
            continue;
        };
        ensure!(
            catalog.last_updated.len() <
                crate::publication::limits::PUBLICATION_ENTRIES as usize,
            "page revisions exceed the publication entry budget"
        );
        catalog
            .last_updated
            .insert(path.value().to_owned(), date.value().to_owned());
    }
    Ok(())
}

/// "Last updated" for a page body, from an RFC 3339 date.
pub(in crate::publication::render::website) fn footer(date: &str) -> String {
    let day = date.get(..10).unwrap_or(date);
    let shown = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|parsed| parsed.format("%-d %B %Y").to_string())
        .unwrap_or_else(|_| day.to_owned());
    format!(
        "<p class=\"page-updated\">Last updated <time \
         datetime=\"{}\">{}</time></p>",
        crate::publication::render::markup::html_escape(date),
        crate::publication::render::markup::html_escape(&shown)
    )
}
