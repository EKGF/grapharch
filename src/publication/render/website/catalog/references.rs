//! Index pinned references without joining independent contribution
//! memberships.
use {
    super::{Catalog, Plan, node},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{HashMap, HashSet},
};

#[derive(Default)]
struct Owners {
    // bounded: at most LINK_INDEX_ENTRIES associations and
    // REFERENCE_INDEX_BYTES.
    entries:      HashMap<Term, HashSet<NamedNode>>,
    associations: usize,
    bytes:        usize,
}
impl Owners {
    fn insert(&mut self, source: Term, owner: NamedNode) -> Result<()> {
        if self
            .entries
            .get(&source)
            .is_some_and(|owners| owners.contains(&owner))
        {
            return Ok(());
        }
        self.associations += 1;
        self.bytes += source.to_string().len() + owner.as_str().len();
        ensure!(
            self.associations <= crate::publication::limits::LINK_INDEX_ENTRIES &&
                self.bytes <=
                    crate::publication::limits::REFERENCE_INDEX_BYTES,
            "website reference ownership exceeds budget"
        );
        self.entries.entry(source).or_default().insert(owner);
        Ok(())
    }
}

pub(super) async fn read(plan: &Plan, catalog: &mut Catalog) -> Result<()> {
    let parents = parents(plan, catalog).await?;
    let owners = references(plan, catalog, &parents).await?;
    // Parent relations are no longer needed; never retain source bodies here.
    drop(parents);
    let fields = plan.source.pattern(
        &format!(
            "?ref <http://purl.org/dc/terms/identifier> ?original ; {} ?url . \
             OPTIONAL {{ ?ref {} ?path }} OPTIONAL {{ ?ref {} ?asset }} \
             OPTIONAL {{ ?ref {} ?presentation }}",
            crate::publication::term("url"),
            crate::publication::term("sourcePath"),
            crate::publication::term("asset"),
            crate::publication::term("imagePresentation")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?ref ?original ?url ?path ?asset ?presentation \
             WHERE {{ {fields} }}"
        ))
        .await?;
    let mut bytes = 0usize;
    while let Some(row) = rows.next().await? {
        let Some(pages) = owners.entries.get(&row["ref"]) else {
            continue
        };
        let Term::Literal(original) = &row["original"] else {
            anyhow::bail!("reference identifier must be literal")
        };
        let Term::Literal(url) = &row["url"] else {
            anyhow::bail!("reference URL must be literal")
        };
        let path = match row.get("path") {
            Some(Term::Literal(v)) => v.value().to_owned(),
            None => String::new(),
            _ => anyhow::bail!("reference path must be literal"),
        };
        let asset = row
            .get("asset")
            .map(node)
            .transpose()?
            .map(|id| id.as_str().to_owned());
        let interactive = if let Some(presentation) = row.get("presentation") {
            ensure!(
                matches!(presentation, Term::Literal(value) if value.value() == "interactive-svg") &&
                    asset.is_some(),
                "invalid SVG image presentation hint"
            );
            true
        } else {
            false
        };
        for owner in pages {
            bytes += original.value().len() + url.value().len() + path.len();
            ensure!(
                bytes <= crate::publication::limits::REFERENCE_INDEX_BYTES &&
                    catalog.references.len() <
                        crate::publication::limits::LINK_INDEX_ENTRIES,
                "website reference index exceeds budget"
            );
            let key = (owner.as_str().into(), original.value().into());
            if interactive {
                catalog.presentations.insert(key.clone());
            }
            let value = (path.clone(), url.value().into(), asset.clone());
            if let Some(previous) =
                catalog.references.insert(key, value.clone())
            {
                ensure!(
                    previous == value,
                    "conflicting website reference metadata"
                );
            }
        }
    }
    Ok(())
}

async fn parents(plan: &Plan, catalog: &Catalog) -> Result<Owners> {
    let pattern = plan.source.pattern(
        &format!(
            "?source {} ?page",
            crate::publication::term("partOf")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?source ?page WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut owners = Owners::default();
    while let Some(row) = rows.next().await? {
        let Term::NamedNode(page) = &row["page"] else {
            continue
        };
        if catalog.pages.contains_key(page.as_str()) {
            owners.insert(row["source"].clone(), page.clone())?;
        }
    }
    Ok(owners)
}
async fn references(
    plan: &Plan,
    catalog: &Catalog,
    parents: &Owners,
) -> Result<Owners> {
    let pattern = plan.source.pattern(
        &format!(
            "?source {} ?ref",
            crate::publication::term("reference")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?source ?ref WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut owners = Owners::default();
    while let Some(row) = rows.next().await? {
        if let Term::NamedNode(source) = &row["source"] &&
            catalog.pages.contains_key(source.as_str())
        {
            owners.insert(row["ref"].clone(), source.clone())?;
        }
        if let Some(pages) = parents.entries.get(&row["source"]) {
            for page in pages {
                owners.insert(row["ref"].clone(), page.clone())?;
            }
        }
    }
    Ok(owners)
}
