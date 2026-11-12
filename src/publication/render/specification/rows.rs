use {
    super::{
        super::{markup::html_escape, model::Plan},
        metadata::{URL_FIELDS, explicit_title, link, optional, people},
    },
    crate::publication::{
        native_html::{self, Links},
        validation::unsigned,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeSet, HashSet},
};

/// Release fields whose value is the target's literal.
const LITERAL_FIELDS: &[&str] =
    &["releaseVersion", "documentNumber", "publicationStatus"];
const PEOPLE_FIELDS: &[&str] =
    &["editor", "formerEditor", "author", "contributor"];

/// The value of one ordered metadata row, read from the target.
pub(in crate::publication::render) enum RowValue {
    /// A literal release field.
    Text(String),
    /// An HTTP(S) publication link with its display label.
    Link { url: String, label: String },
    /// Contributors in one role; HTML renders their full details.
    People { field: String, names: Vec<String> },
    /// Source-owned HTML for a `custom` row.
    Html(String),
}

/// One `ga:metadataRow` with its label from the definitions.
pub(in crate::publication::render) struct MetadataRow {
    pub label: String,
    pub value: RowValue,
}

/// Read the target's ordered rows and check the profile's required fields.
pub(in crate::publication::render) async fn read(
    plan: &Plan,
) -> Result<Vec<MetadataRow>> {
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
    fields.sort();
    let mut result = Vec::with_capacity(fields.len());
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
            // Relative links name artifacts beside this publication. They stay
            // relative to the page; ga:thisVersion is not their base.
            let fragment = native_html::read(
                &body,
                Links::Publication(&plan.targets),
                &mut |_| Ok(None),
            )?;
            ensure!(
                fragment.ids.is_empty() && fragment.headings.is_empty(),
                "metadata rows cannot introduce document anchors or headings"
            );
            result.push(MetadataRow {
                label,
                value: RowValue::Html(fragment.html),
            });
            continue;
        }
        ensure!(
            seen.insert(field.clone()),
            "duplicate metadata field {field}"
        );
        let value = if PEOPLE_FIELDS.contains(&field.as_str()) {
            RowValue::People {
                names: super::super::people::read(plan, &field)
                    .await?
                    .into_iter()
                    .map(|person| person.name)
                    .collect(),
                field,
            }
        } else if LITERAL_FIELDS.contains(&field.as_str()) {
            RowValue::Text(
                optional(plan, &plan.target, &field).await?.ok_or_else(
                    || anyhow::anyhow!("metadata row {field} has no value"),
                )?,
            )
        } else {
            ensure!(
                URL_FIELDS.iter().any(|(name, _, _)| *name == field),
                "unsupported metadata field {field}"
            );
            let uri = plan.source.node(&plan.target, &field).await?;
            let label = optional(plan, &row, "linkLabel")
                .await?
                .unwrap_or_else(|| uri.as_str().to_owned());
            super::metadata::validate_url(uri.as_str())?;
            RowValue::Link { url: uri.as_str().to_owned(), label }
        };
        result.push(MetadataRow { label, value });
    }
    let required = required_fields(plan).await?;
    let missing = required
        .iter()
        .filter(|field| !seen.contains(*field))
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        missing.is_empty(),
        "profile requires ordered metadata rows for: {}",
        missing.join(", ")
    );
    Ok(result)
}

/// `ga:requiredMetadataField` values declared on the profile.
async fn required_fields(plan: &Plan) -> Result<BTreeSet<String>> {
    // bounded: at most 16 required fields.
    let mut fields = BTreeSet::new();
    let mut rows = plan
        .source
        .objects(&plan.profile, "requiredMetadataField")
        .await?;
    while let Some(row) = rows.next().await? {
        let Term::Literal(field) = &row["value"] else {
            anyhow::bail!("requiredMetadataField must be a literal")
        };
        ensure!(
            fields.len() < 16,
            "profile exceeds 16 required metadata fields"
        );
        fields.insert(field.value().to_owned());
    }
    Ok(fields)
}

pub(super) async fn render(plan: &Plan) -> Result<Option<String>> {
    let rows = read(plan).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut html = String::new();
    for row in rows {
        match row.value {
            RowValue::Html(body) => {
                html.push_str(&format!(
                    "<dt>{}</dt><dd>{body}</dd>",
                    html_escape(&row.label)
                ));
            },
            RowValue::Text(value) => {
                html.push_str(&format!(
                    "<dt>{}</dt><dd>{}</dd>",
                    html_escape(&row.label),
                    html_escape(&value)
                ));
            },
            RowValue::Link { url, label } => {
                html.push_str(&format!(
                    "<dt>{}</dt><dd>{}</dd>",
                    html_escape(&row.label),
                    link(&url, &label)?
                ));
            },
            RowValue::People { field, .. } => {
                html.push_str(
                    &people(plan, &field, "", "", Some(&row.label)).await?,
                );
            },
        }
    }
    Ok(Some(html))
}
