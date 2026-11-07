//! One bounded metadata read per selected fragment, without retaining all
//! bodies.
use {
    crate::publication::{RDF_TYPE, render::model::Plan, term},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
pub(super) struct Fragment {
    pub owner:    NamedNode,
    pub body:     String,
    pub subjects: Vec<Term>,
    pub paths:    Vec<Term>,
}
impl Fragment {
    pub async fn read(plan: &Plan, fragment: &NamedNode) -> Result<Self> {
        let pattern = plan
            .source
            .pattern(&format!("{fragment} ?predicate ?value"), 0);
        let mut rows = plan
            .dataset
            .select(format!(
                "SELECT DISTINCT ?predicate ?value WHERE {{ VALUES ?predicate \
                 {{ <{RDF_TYPE}> {} {} {} {} }} {pattern} }}",
                term("partOf"),
                term("body"),
                term("subject"),
                term("sourcePath")
            ))
            .await?;
        let mut text_asset = false;
        let mut owner = None;
        let mut ambiguous_owner = false;
        let mut body = None;
        let mut subjects = Vec::new(); // bounded: selected metadata, at most LINK_INDEX_ENTRIES terms and REFERENCE_INDEX_BYTES bytes.
        let mut paths = Vec::new(); // bounded by the same shared per-fragment metadata budget.
        let mut count = 0;
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            let value = &row["value"];
            bytes += match value {
                Term::Literal(value) => value.value().len(),
                Term::NamedNode(value) => value.as_str().len(),
                value => value.to_string().len(),
            };
            ensure!(
                count <= crate::publication::limits::LINK_INDEX_ENTRIES &&
                    bytes <=
                        crate::publication::limits::REFERENCE_INDEX_BYTES +
                            1024 * 1024,
                "fragment metadata exceeds its bounded index: {fragment}"
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                anyhow::bail!("fragment metadata predicate must be named")
            };
            if predicate.as_str() == RDF_TYPE {
                text_asset |= value == &Term::NamedNode(term("TextAsset"));
            } else if predicate == &term("body") {
                ensure!(body.is_none(), "{fragment} conflicting body");
                let Term::Literal(value) = value else {
                    anyhow::bail!("{fragment} missing or invalid body literal")
                };
                ensure!(
                    value.value().len() <= 1024 * 1024,
                    "website fragment exceeds 1 MiB page budget"
                );
                body = Some(value.value().to_owned());
            } else if predicate == &term("partOf") {
                ambiguous_owner |= owner.is_some();
                owner = Some(value.clone());
            } else if predicate == &term("subject") {
                subjects.push(value.clone());
            } else if predicate == &term("sourcePath") {
                paths.push(value.clone());
            }
        }
        let owner = if text_asset {
            fragment.clone()
        } else {
            ensure!(!ambiguous_owner, "{fragment} conflicting partOf");
            let Some(Term::NamedNode(owner)) = owner else {
                anyhow::bail!("{fragment} missing or invalid partOf IRI")
            };
            owner
        };
        Ok(Self {
            owner,
            body: body.ok_or_else(|| {
                anyhow::anyhow!("{fragment} missing or invalid body literal")
            })?,
            subjects,
            paths,
        })
    }
}
