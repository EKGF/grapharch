//! Showcase cards are selected from project resources in the pinned KG.
#[path = "gallery_discovery.rs"]
mod discovery;

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

#[derive(Default)]
pub(super) struct Gallery {
    pub html:    String,
    pub details: Vec<Detail>,
}

pub(super) struct Detail {
    pub slug:  String,
    pub title: String,
    pub body:  String,
}

pub(super) async fn render(
    plan: &Plan,
    catalog: &Catalog,
    page: &NamedNode,
    prefix: &str,
) -> Result<Gallery> {
    match plan.source.optional(page, "showcaseGallery").await? {
        None => return Ok(Gallery::default()),
        Some(Term::Literal(flag)) if flag.value() == "true" => {},
        _ => anyhow::bail!("showcaseGallery must be true when declared"),
    }
    let mut rows = plan.source.objects(&plan.project, "showcase").await?;
    let mut details = Vec::new(); // bounded: one detail page per at most 16 showcases.
    let mut cards = BTreeMap::new(); // bounded: at most 16 registered showcases.
    let mut facets = discovery::Discovery::default();
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
        let mut discovery = discovery::Discovery::read(plan, id).await?;
        discovery.outputs.insert("KG".into());
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
        let abstract_text = match plan
            .source
            .optional_by(
                id,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/abstract"),
            )
            .await?
        {
            None => String::new(),
            Some(Term::Literal(text)) => {
                ensure!(
                    text.value().len() <= 16384,
                    "showcase abstract exceeds 16 KiB"
                );
                format!(
                    "<p class=\"showcase-background\">{}</p>",
                    esc(text.value())
                )
            },
            _ => anyhow::bail!("showcase abstract must be a literal"),
        };
        let preview = plan.source.node(id, "previewArtifact").await?;
        let pdf = match plan.source.optional(id, "pdfArtifact").await? {
            None => None,
            Some(Term::NamedNode(pdf)) => {
                let artifact = catalog.page(&pdf)?;
                ensure!(
                    artifact.binary,
                    "showcase PDF must be a selected artifact"
                );
                Some(artifact.route.clone())
            },
            _ => anyhow::bail!("showcase PDF must be an artifact IRI"),
        };
        let dataset = plan.source.node(id, "datasetArtifact").await?;
        let preview = catalog.page(&preview)?;
        let dataset = catalog.page(&dataset)?;
        ensure!(
            preview.binary && dataset.binary,
            "showcase outputs must be selected publication artifacts"
        );
        let count: u32 =
            plan.source.text(id, "diagnosticCount").await?.parse()?;
        let badge = diagnostic_badge(count);
        let mut alternatives = BTreeMap::new();
        let mut pdfs = BTreeMap::new();
        let mut routes = std::collections::BTreeSet::new(); // bounded: at most 32 outputs.
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
            ensure!(
                routes.insert(artifact.route.clone()),
                "duplicate showcase output route"
            );
            let output_title = title(&plan.source, output).await?;
            if artifact.route.ends_with(".html") {
                discovery.outputs.insert("HTML".into());
            }
            if artifact.route.ends_with(".pdf") {
                discovery.outputs.insert("PDF".into());
                pdfs.insert(
                    order,
                    (artifact.route.clone(), output_title.clone()),
                );
            }
            let format = if artifact.route.ends_with(".pdf") {
                "PDF"
            } else if artifact.route.ends_with(".html") {
                "HTML"
            } else {
                "Artifact"
            };
            alternatives.insert(
                order,
                format!(
                    "<li><a data-artifact href=\"{}\"><span \
                     class=\"showcase-output-format\">{format}</\
                     span><span>{}</span></a></li>",
                    esc(&artifact.route),
                    esc(&output_title)
                ),
            );
        }
        if let Some(pdf) = pdf {
            ensure!(
                pdfs.values().any(|(route, _)| route == &pdf),
                "primary showcase PDF must be a declared output"
            );
        }
        ensure!(
            routes.contains(&preview.route),
            "primary showcase HTML must be a declared output"
        );
        let alternatives = alternatives.into_values().collect::<String>();
        let attributes = discovery.attributes();
        let chips = discovery.chips();
        let credits = if discovery.credits.is_empty() {
            String::new()
        } else {
            format!(
                "<details class=\"showcase-credits\"><summary>Source and \
                 credits</summary>{}</details>",
                discovery.credits
            )
        };
        facets.merge(&discovery);
        let slug = plan.source.text(id, "slug").await?;
        ensure!(
            !slug.is_empty() &&
                slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid showcase detail slug"
        );
        let slug = format!("showcase-{slug}");
        details.push(Detail {
            slug: slug.clone(),
            title: name.clone(),
            body: format!(r#"<article class="showcase-detail" data-showcase="{id}"{attributes}><h1>{name}</h1>{chips}<p>{description}</p>{abstract_text}{badge}<h2>Explore this publication</h2><ul class="showcase-outputs">{alternatives}<li><a data-artifact href="{dataset}" download><span class="showcase-output-format">KG</span><span>Download documentation graph</span></a></li></ul>{credits}</article>"#,
                id=esc(id.as_str()), name=esc(&name), description=esc(&description), dataset=esc(&dataset.route)),
        });
        cards.insert(order, format!(r#"<article class="showcase-card" data-showcase="{id}"{attributes}><a class="showcase-primary" href="{prefix}{slug}.html" aria-labelledby="showcase-title-{order}"><div class="showcase-preview"><iframe sandbox="" loading="lazy" tabindex="-1" title="Preview of {name}" src="{prefix}{preview}"></iframe></div><div class="showcase-info"><h2 id="showcase-title-{order}">{name}</h2>{chips}<p>{description}</p>{badge}</div></a></article>"#,
            id=esc(id.as_str()), name=esc(&name), description=esc(&description), preview=esc(&preview.route)));
    }
    let controls = facets.controls(cards.len());
    Ok(Gallery {
        details,
        html: format!(
            "<section data-showcase-gallery aria-label=\"Publication \
             examples\">{controls}<div \
             class=\"showcase-gallery\">{}</div></section><script>{}</script>",
            cards.into_values().collect::<String>(),
            include_str!("gallery.js")
        ),
    })
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
