//! Section panels selected by heading anchor.
use {
    super::super::super::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

pub(in crate::publication::render::website) async fn panels(
    plan: &Plan,
    page: &NamedNode,
) -> Result<Vec<String>> {
    // bounded: one page can select at most 32 section panels.
    let mut panels = Vec::new();
    let mut rows = plan.source.objects(page, "panelSection").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            panels.len() < 32,
            "page exceeds 32 panel sections"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("panel section must be a heading anchor literal")
        };
        let anchor = value.value();
        ensure!(
            !anchor.is_empty() &&
                anchor.len() <= 128 &&
                anchor
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_')),
            "invalid panel section anchor {anchor}"
        );
        panels.push(anchor.to_owned());
    }
    Ok(panels)
}
