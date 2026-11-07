//! Isolated, source-owned SVG navigation projected from the pinned publication
//! KG.
mod projection;
use {
    super::catalog::{Catalog, Page},
    crate::publication::render::{
        image_data,
        markup::html_escape as esc,
        model::Plan,
    },
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, TagEnd},
    sha2::{Digest, Sha256},
    std::{collections::BTreeSet, path::Path},
};
#[derive(Clone)]
pub(super) struct Projection {
    pub route:  String,
    pub aspect: String,
}
pub(super) async fn prepare(
    plan: &Plan,
    catalog: &mut Catalog,
    stage: &Path,
) -> Result<Vec<String>> {
    let mut selected = BTreeSet::new(); // bounded: source reference index and 256 selected image assets.
    for key in &catalog.presentations {
        let asset = catalog
            .references
            .get(key)
            .and_then(|value| value.2.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!("interactive image has no retained asset")
            })?;
        selected.insert(asset.clone());
        ensure!(
            selected.len() <= super::budget::PROJECTIONS,
            "interactive SVG asset count exceeds budget"
        );
    }
    let mut files = BTreeSet::new();
    let mut total = 0usize;
    for asset in selected {
        let mut context = catalog
            .pages
            .get(&asset)
            .ok_or_else(|| {
                anyhow::anyhow!("interactive SVG asset is not selected")
            })?
            .clone();
        let mime = plan.source.text(&context.id, "format").await?;
        ensure!(
            mime == "image/svg+xml",
            "interactive SVG presentation requires an SVG image"
        );
        let bytes = image_data::read(plan, &context.id, &mime).await?;
        total += bytes.len();
        ensure!(
            total <=
                crate::publication::image_assets::MAX_PUBLICATION_IMAGE_BYTES,
            "interactive images exceed total byte budget"
        );
        context.route = "assets/interactive.svg".into();
        let projection = projection::project(&bytes, |value| {
            super::markdown::link(value, &context, catalog)
        })?;
        let route = format!(
            "assets/interactive-{}.svg",
            hex::encode(Sha256::digest(&projection.bytes))
        );
        if files.insert(route.clone()) {
            tokio::fs::create_dir_all(stage.join("assets")).await?;
            tokio::fs::write(stage.join(&route), projection.bytes).await?;
        }
        catalog.projections.insert(asset, Projection {
            route,
            aspect: projection.aspect,
        });
    }
    Ok(files.into_iter().collect())
}
pub(super) fn replace<'a>(
    events: &mut [Event<'a>],
    index: usize,
    projection: Option<&Projection>,
    path: &str,
    title: Option<&str>,
    scheme: Option<crate::publication::image_hints::ColorScheme>,
) -> Result<()> {
    let mut alt = String::new();
    let mut end = None;
    for (relative, event) in events[index + 1..].iter().enumerate() {
        match event {
            Event::End(TagEnd::Image) => {
                end = Some(index + 1 + relative);
                break;
            },
            Event::Text(text) | Event::Code(text) => alt.push_str(text),
            Event::SoftBreak | Event::HardBreak => alt.push(' '),
            _ => {},
        }
    }
    ensure!(
        alt.len() <= 1024 && (projection.is_none() || !alt.trim().is_empty()),
        "interactive SVG requires nonempty alternative text of at most 1024 \
         bytes"
    );
    let end = end.ok_or_else(|| anyhow::anyhow!("unclosed SVG image"))?;
    let title = title
        .map(|value| format!(" title=\"{}\"", esc(value)))
        .unwrap_or_default();
    let scheme = scheme.map(|scheme| {
        match scheme {
            crate::publication::image_hints::ColorScheme::Light => "light",
            crate::publication::image_hints::ColorScheme::Dark => "dark",
        }
    });
    let attributes = scheme
        .map(|value| format!(" data-image-scheme=\"{value}\""))
        .unwrap_or_default();
    let html = if let Some(projection) = projection {
        format!(
            "<span class=\"interactive-svg-frame\" \
             style=\"aspect-ratio:{}\"{}><object class=\"interactive-svg\" \
             type=\"image/svg+xml\" data=\"{}\" aria-label=\"{}\" \
             style=\"display:block;width:100%;height:100%;color-scheme:\
             inherit\"{}><a href=\"{}\">{}</a></object></span>",
            esc(&projection.aspect),
            attributes,
            esc(path),
            esc(&alt),
            title,
            esc(path),
            esc(&alt)
        )
    } else {
        format!(
            "<img class=\"image-color-variant\"{} src=\"{}\" alt=\"{}\"{}>",
            attributes,
            esc(path),
            esc(&alt),
            title
        )
    };
    events[index] = Event::InlineHtml(html.into());
    for event in &mut events[index + 1..=end] {
        *event = Event::InlineHtml("".into());
    }
    Ok(())
}
pub(super) fn path(page: &Page, projection: &Projection) -> String {
    format!(
        "{}{}",
        "../".repeat(page.route.matches('/').count()),
        projection.route
    )
}
