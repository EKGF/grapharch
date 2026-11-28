//! The decide pass: every reference recorded at collection is classified
//! once, before any page renders; rendering only applies the decisions.
use {
    super::{Catalog, Plan},
    crate::publication::{
        reference::{Classification, ReferenceClassifier},
        term,
    },
    anyhow::{Result, ensure},
    oxrdf::Term,
    std::collections::HashMap,
};

/// Decisions keyed by page source and original reference.
pub(in crate::publication::render::website) type Decisions =
    HashMap<(String, String), Classification>;

/// Read the recorded references and their repository-file evidence.
pub(super) async fn read(plan: &Plan, catalog: &mut Catalog) -> Result<()> {
    let pattern = plan.source.pattern(
        &format!(
            "?reference a {} ; {} ?source ; {} ?original . OPTIONAL {{ \
             ?reference {} ?exists }}",
            term("Reference"),
            term("referenceSource"),
            term("originalReference"),
            term("repositoryFileExists")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?source ?original ?exists WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut recorded = Vec::new(); // bounded: LINK_INDEX_ENTRIES references.
    while let Some(row) = rows.next().await? {
        ensure!(
            recorded.len() < crate::publication::limits::LINK_INDEX_ENTRIES,
            "recorded references exceed the link index budget"
        );
        let (Term::Literal(source), Term::Literal(original)) =
            (&row["source"], &row["original"])
        else {
            anyhow::bail!("reference source and original must be literals")
        };
        let key = (
            source.value().to_owned(),
            original.value().to_owned(),
        );
        if let Some(Term::Literal(exists)) = row.get("exists") {
            catalog
                .repository_files
                .insert(key.clone(), exists.value() == "true");
        }
        recorded.push(key);
    }
    let pages: HashMap<&str, &str> = catalog
        .sources
        .iter()
        .map(|(source, id)| (source.as_str(), id.as_str()))
        .collect(); // bounded: one entry per selected page.
    let mut decisions = Decisions::new(); // bounded: recorded references.
    for (source, original) in recorded {
        let Some(page) = pages
            .get(source.as_str())
            .and_then(|id| catalog.pages.get(*id))
        else {
            continue;
        };
        if page.asset {
            continue;
        }
        let lookup =
            super::super::reference_lookup::PageLookup { page, catalog };
        // A broken reference under the strict policy fails when the page
        // renders, with the page's own context; record only decisions here.
        if let Ok(classification) = ReferenceClassifier::new(
            &lookup,
            catalog.link_policy,
        )
        .classify(&original, &page.source, &page.route, false)
        {
            decisions.insert((source, original), classification);
        }
    }
    catalog.decisions = decisions;
    Ok(())
}
