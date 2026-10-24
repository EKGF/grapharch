use {
    super::{
        super::{markup::html_escape, model::Plan},
        metadata::{URL_FIELDS, explicit_title, link, optional, people},
    },
    crate::publication::{native_html, validation::unsigned},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::HashSet,
};

pub(super) async fn render(plan: &Plan) -> Result<Option<String>> {
    let mut rows = plan.source.objects(&plan.target, "metadataRow").await?;
    let mut fields: Vec<(u32, NamedNode)> = Vec::new();
    let mut orders = HashSet::new();
    while let Some(row) = rows.next().await? {
        ensure!(
            fields.len() < 64,
            "specification exceeds 64 metadata rows"
        );
        let Term::NamedNode(id) = &row["value"] else {
            anyhow::bail!("metadataRow must be an IRI")
        };
        let order = unsigned(
            &plan.source.optional(id, "order").await?.ok_or_else(|| {
                anyhow::anyhow!("metadata row requires order")
            })?,
            "metadata row order",
        )?;
        ensure!(
            orders.insert(order),
            "metadata row orders must be distinct"
        );
        fields.push((order, id.clone()));
    }
    if fields.is_empty() {
        return Ok(None);
    }
    fields.sort();
    let mut html = String::new();
    let mut seen = HashSet::new();
    for (_, row) in fields {
        let field = plan.source.text(&row, "field").await?;
        let label = explicit_title(plan, &row).await?;
        if field == "custom" {
            let body = plan.source.text(&row, "body").await?;
            ensure!(
                body.len() <= 64 * 1024,
                "custom metadata row exceeds 64 KiB"
            );
            let fragment = native_html::read(&body, None, &mut |_| Ok(None))?;
            ensure!(
                fragment.ids.is_empty() && fragment.headings.is_empty(),
                "metadata rows cannot introduce document anchors or headings"
            );
            html.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                html_escape(&label),
                fragment.html
            ));
            continue;
        }
        ensure!(
            seen.insert(field.clone()),
            "duplicate metadata field {field}"
        );
        if matches!(
            field.as_str(),
            "editor" | "formerEditor" | "author"
        ) {
            html.push_str(&people(plan, &field, "", "", Some(&label)).await?);
        } else {
            ensure!(
                URL_FIELDS.iter().any(|(name, _, _)| *name == field),
                "unsupported metadata field {field}"
            );
            let uri = plan.source.node(&plan.target, &field).await?;
            let display = optional(plan, &row, "linkLabel")
                .await?
                .unwrap_or_else(|| uri.as_str().to_owned());
            html.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                html_escape(&label),
                link(uri.as_str(), &display)?
            ));
        }
    }
    ensure!(
        seen.contains("thisVersion") && seen.contains("latestVersion"),
        "ordered metadata must include thisVersion and latestVersion"
    );
    Ok(Some(html))
}
