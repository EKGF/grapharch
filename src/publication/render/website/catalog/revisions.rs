//! The Git dates of each page source, recorded at collection: the newest
//! change and the creation, shown as relative times with the absolute date
//! in a tooltip, as Material's git-revision-date-localized facts.
use {
    super::{Catalog, Plan},
    crate::publication::{render::markup::html_escape as esc, term},
    anyhow::{Result, ensure},
    chrono::{DateTime, FixedOffset},
    oxrdf::Term,
};

/// The dates of one page source.
#[derive(Clone, Debug)]
pub(in crate::publication::render::website) struct PageDates {
    modified: DateTime<FixedOffset>,
    created:  Option<DateTime<FixedOffset>>,
}

/// One date fact: its icon, label and date.
struct Fact<'a> {
    icon:  &'static str,
    label: &'static str,
    date:  &'a DateTime<FixedOffset>,
}

/// Read `ga:PageRevision` dates into the catalog, keyed by source path.
pub(super) async fn read(plan: &Plan, catalog: &mut Catalog) -> Result<()> {
    let pattern = plan.source.pattern(
        &format!(
            "?revision a {} ; {} ?path ; <http://purl.org/dc/terms/modified> \
             ?modified . OPTIONAL {{ ?revision \
             <http://purl.org/dc/terms/created> ?created }}",
            term("PageRevision"),
            term("sourcePath")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?path ?modified ?created WHERE {{ {pattern} }}"
        ))
        .await?;
    while let Some(row) = rows.next().await? {
        let (Term::Literal(path), Term::Literal(modified)) =
            (&row["path"], &row["modified"])
        else {
            continue;
        };
        let created = match row.get("created") {
            Some(Term::Literal(created)) => Some(parse(created.value())?),
            _ => None,
        };
        ensure!(
            catalog.last_updated.len() <
                crate::publication::limits::PUBLICATION_ENTRIES as usize,
            "page revisions exceed the publication entry budget"
        );
        catalog
            .last_updated
            .insert(path.value().to_owned(), PageDates {
                modified: parse(modified.value())?,
                created,
            });
    }
    Ok(())
}

fn parse(value: &str) -> Result<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|error| anyhow::anyhow!("invalid page date {value}: {error}"))
}

impl PageDates {
    /// The page's date facts: last update, then creation. A script replaces
    /// each shown date with its relative time; the absolute date stays in
    /// the tooltip and the `datetime` attribute.
    pub(in crate::publication::render::website) fn html(&self) -> String {
        let mut html = String::from("<aside class=\"page-dates\">");
        let facts = [
            Some(Fact {
                icon:  "history",
                label: "Last update",
                date:  &self.modified,
            }),
            self.created.as_ref().map(|date| {
                Fact { icon: "clock-plus", label: "Created", date }
            }),
        ];
        for fact in facts.iter().flatten() {
            html.push_str(&fact.html());
        }
        html.push_str("</aside>");
        html
    }
}

impl Fact<'_> {
    fn html(&self) -> String {
        let shown = self.date.format("%-d %B %Y").to_string();
        let title = format!("{}: {shown}", self.label);
        format!(
            "<span class=\"page-date\" title=\"{title}\"><span \
             class=\"page-date-icon\" aria-hidden=\"true\">{icon}</span><span \
             class=\"visually-hidden\">{label}: </span><time datetime=\"{}\" \
             data-relative-time>{shown}</time></span>",
            esc(&self.date.to_rfc3339()),
            title = esc(&title),
            icon = super::super::project::icon_svg(self.icon)
                .expect("bundled date icon"),
            label = esc(self.label),
            shown = esc(&shown),
        )
    }
}
