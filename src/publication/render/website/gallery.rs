//! Showcase cards are selected from project resources in the pinned KG.
use {
    super::{
        super::{
            markup::html_escape as esc,
            model::{Plan, title},
        },
        catalog::Catalog,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

pub(super) async fn render(
    plan: &Plan,
    catalog: &Catalog,
    page: &NamedNode,
    prefix: &str,
) -> Result<String> {
    match plan.source.optional(page, "showcaseGallery").await? {
        None => return Ok(String::new()),
        Some(Term::Literal(flag)) if flag.value() == "true" => {},
        _ => anyhow::bail!("showcaseGallery must be true when declared"),
    }
    let mut rows = plan.source.objects(&plan.project, "showcase").await?;
    let mut cards = BTreeMap::new();
    while let Some(row) = rows.next().await? {
        let Term::NamedNode(id) = &row["value"] else {
            anyhow::bail!("showcase must be an IRI")
        };
        ensure!(
            plan.source.has_type(id, "Showcase").await?,
            "undefined showcase"
        );
        let order: u32 = plan.source.text(id, "order").await?.parse()?;
        ensure!(
            cards.len() < 16 && !cards.contains_key(&order),
            "showcase ordering is ambiguous or exceeds budget"
        );
        let name = title(&plan.source, id).await?;
        let description = match plan
            .source
            .optional_by(
                id,
                &NamedNode::new_unchecked(
                    "http://purl.org/dc/terms/description",
                ),
            )
            .await?
        {
            Some(Term::Literal(text)) => text.value().to_owned(),
            _ => anyhow::bail!("showcase description is required"),
        };
        let preview = plan.source.node(id, "previewArtifact").await?;
        let pdf = plan.source.node(id, "pdfArtifact").await?;
        let dataset = plan.source.node(id, "datasetArtifact").await?;
        let preview = catalog.page(&preview)?;
        let pdf = catalog.page(&pdf)?;
        let dataset = catalog.page(&dataset)?;
        ensure!(
            preview.binary && pdf.binary && dataset.binary,
            "showcase outputs must be selected publication artifacts"
        );
        let count: u32 =
            plan.source.text(id, "diagnosticCount").await?.parse()?;
        let badge = diagnostic_badge(count);
        let mut alternatives = BTreeMap::new();
        let mut pdfs = BTreeMap::new();
        let mut outputs = plan.source.objects(id, "outputArtifact").await?;
        while let Some(row) = outputs.next().await? {
            let Term::NamedNode(output) = &row["value"] else {
                anyhow::bail!("showcase output must be an IRI")
            };
            let artifact = catalog.page(output)?;
            let order: u32 =
                plan.source.text(output, "order").await?.parse()?;
            ensure!(
                alternatives.len() < 32 && !alternatives.contains_key(&order),
                "showcase outputs have ambiguous order or exceed budget"
            );
            let output_title = title(&plan.source, output).await?;
            if artifact.route.ends_with(".pdf") {
                pdfs.insert(
                    order,
                    (artifact.route.clone(), output_title.clone()),
                );
            }
            alternatives.insert(
                order,
                format!(
                    "<li><a data-artifact href=\"{prefix}{}\">{}</a></li>",
                    esc(&artifact.route),
                    esc(&output_title)
                ),
            );
        }
        ensure!(
            !pdfs.is_empty(),
            "showcase must expose its PDF output"
        );
        ensure!(
            pdfs.values().any(|(route, _)| route == &pdf.route),
            "primary showcase PDF must be a declared output"
        );
        let multiple = pdfs.len() > 1;
        let downloads = pdfs
            .into_values()
            .map(|(route, title)| {
                let label = if multiple { title.as_str() } else { "PDF" };
                super::super::pdf_download::link(
                    &format!("{prefix}{route}"),
                    label,
                    "showcase-pdf",
                )
                .replacen("<a ", "<a data-artifact ", 1)
            })
            .collect::<String>();
        let alternatives = alternatives.into_values().collect::<String>();
        cards.insert(order, format!(r#"<article class="showcase-card" data-showcase="{id}"><div class="showcase-preview"><iframe sandbox="" loading="lazy" tabindex="-1" title="Preview of {name}" src="{prefix}{preview}"></iframe></div><div class="showcase-info"><h2>{name}</h2><p>{description}</p>{badge}<div class="showcase-actions"><a data-artifact class="showcase-open" href="{prefix}{preview}">Open HTML</a>{downloads}<a data-artifact href="{prefix}{dataset}" download>Download KG</a></div><details><summary>All publication outputs</summary><ul>{alternatives}</ul></details></div></article>"#,
            id=esc(id.as_str()), name=esc(&name), description=esc(&description), preview=esc(&preview.route), dataset=esc(&dataset.route)));
    }
    Ok(format!(
        "<div class=\"showcase-gallery\">{}</div>",
        cards.into_values().collect::<String>()
    ))
}

fn diagnostic_badge(count: u32) -> String {
    if count == 0 {
        return String::new();
    }
    let noun = if count == 1 { "issue" } else { "issues" };
    format!(
        "<span class=\"showcase-diagnostic\">{count} known {noun} in the \
         source document</span>"
    )
}

#[cfg(test)]
mod tests {
    use super::diagnostic_badge;

    #[test]
    fn known_source_issue_badge_has_clear_singular_and_plural_labels() {
        assert!(
            diagnostic_badge(1)
                .contains("1 known issue in the source document")
        );
        assert!(
            diagnostic_badge(2)
                .contains("2 known issues in the source document")
        );
        assert!(diagnostic_badge(0).is_empty());
    }
}
