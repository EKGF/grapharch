//! Authored figures of a family chapter, and the images they show.
//!
//! A definition attaches ordered `ga:Figure` resources to an ontology with
//! `ga:figure`. Each names a retained image (`ga:asset`) and its caption
//! (`dct:title`). The chapter receives one fragment of numbered figures after
//! its introduction; renderers number and caption them.
use {
    super::{identity, q},
    crate::publication::{
        collect::{Collector, LocalImageFile},
        term,
        validation::unsigned,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{Literal, NamedNode, Quad, Term},
    std::{path::Path, sync::Arc},
};

const TITLE: &str = "http://purl.org/dc/terms/title";
const IDENTIFIER: &str = "http://purl.org/dc/terms/identifier";
const IMAGE_COLLECTOR: &str =
    "https://ekgf.org/ontology/grapharch#ImageCollector";
/// bounded: authored figures of one ontology chapter.
const MAX_FIGURES: usize = 64;
/// bounded: image collector uses of one definitions file.
const MAX_IMAGES: usize = 256;

struct Figure {
    asset:   NamedNode,
    caption: String,
    order:   u32,
}

/// The ordered authored figures of one ontology.
pub(super) struct Figures {
    // bounded: at most MAX_FIGURES.
    figures: Vec<Figure>,
}

impl Figures {
    pub async fn read(
        view: &RevisionView,
        ontology: &NamedNode,
    ) -> Result<Self> {
        let mut figures = Vec::new();
        let mut rows = view.objects(ontology, "figure").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(figure) = &row["value"] else {
                bail!("{ontology} ga:figure must be a figure IRI")
            };
            ensure!(
                figures.len() < MAX_FIGURES,
                "{ontology} exceeds {MAX_FIGURES} figures"
            );
            ensure!(
                view.has_type(figure, "Figure").await?,
                "{figure} must be typed ga:Figure"
            );
            let asset = view.node(figure, "asset").await?;
            ensure!(
                view.has_type(&asset, "BinaryAsset").await?,
                "figure {figure} must show a retained image asset"
            );
            let caption = match view
                .optional_by(figure, &NamedNode::new_unchecked(TITLE))
                .await?
            {
                Some(Term::Literal(title))
                    if !title.value().trim().is_empty() &&
                        title.value().len() <= 1024 =>
                {
                    title.value().trim().to_owned()
                },
                _ => bail!("figure {figure} requires a caption dct:title"),
            };
            let order = unsigned(
                &view.optional(figure, "order").await?.ok_or_else(|| {
                    anyhow::anyhow!("figure {figure} requires ga:order")
                })?,
                "figure order",
            )?;
            figures.push(Figure { asset, caption, order });
        }
        figures.sort_by_key(|figure| figure.order);
        ensure!(
            figures
                .windows(2)
                .all(|pair| pair[0].order != pair[1].order),
            "{ontology} has ambiguous figure orders"
        );
        Ok(Self { figures })
    }

    /// The chapter fragment showing every figure, placed at `order`.
    pub fn write(
        &self,
        data: &mut Vec<Quad>,
        target: &NamedNode,
        ontology: &NamedNode,
        contribution: &NamedNode,
        order: u32,
    ) {
        if self.figures.is_empty() {
            return;
        }
        let fragment = identity("family-figure-fragment", &[
            target.as_str(),
            ontology.as_str(),
        ]);
        let mut body = String::new();
        for (index, figure) in self.figures.iter().enumerate() {
            let original = format!("figure-{}", index + 1);
            body.push_str(&format!(
                "![{}]({original} \"grapharch:figure\")\n\n",
                Self::escape(&figure.caption)
            ));
            let reference = identity("family-figure-reference", &[
                fragment.as_str(),
                &original,
            ]);
            data.extend([
                q(&fragment, "reference", reference.clone()),
                q(&fragment, "asset", figure.asset.clone()),
                q(&reference, "type", term("ExternalReference")),
                Quad::new(
                    reference.clone(),
                    NamedNode::new_unchecked(IDENTIFIER),
                    Literal::new_simple_literal(original),
                    oxrdf::GraphName::DefaultGraph,
                ),
                q(
                    &reference,
                    "url",
                    Literal::new_simple_literal(figure.asset.as_str()),
                ),
                q(&reference, "asset", figure.asset.clone()),
            ]);
        }
        data.extend([
            q(
                &identity("family-component", &[
                    target.as_str(),
                    ontology.as_str(),
                ]),
                "fragment",
                fragment.clone(),
            ),
            q(&fragment, "type", term("Fragment")),
            q(&fragment, "subject", ontology.clone()),
            q(
                &fragment,
                "body",
                Literal::new_simple_literal(body),
            ),
            q(
                &fragment,
                "format",
                Literal::new_simple_literal("text/markdown"),
            ),
            q(&fragment, "contribution", contribution.clone()),
            q(&fragment, "order", Literal::from(order)),
        ]);
    }

    /// Markdown image text that reads back as exactly the caption.
    fn escape(caption: &str) -> String {
        let mut escaped = String::with_capacity(caption.len());
        for character in caption
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
        {
            if character.is_ascii_punctuation() {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        escaped
    }

    /// Collectors for the images that the definitions declare with
    /// `ga:ImageCollector` uses, read from beside the definitions file.
    pub async fn image_collectors(
        view: &RevisionView,
        project: &str,
        definitions: &Path,
    ) -> Result<Vec<Arc<dyn Collector>>> {
        let root = definitions.parent().ok_or_else(|| {
            anyhow::anyhow!("definitions file has no directory")
        })?;
        let mut collectors: Vec<Arc<dyn Collector>> = Vec::new();
        let mut uses =
            view.objects(&NamedNode::new(project)?, "collector").await?;
        while let Some(row) = uses.next().await? {
            let Term::NamedNode(usage) = &row["value"] else {
                bail!("project ga:collector must be an IRI")
            };
            if view.node(usage, "collectorType").await?.as_str() !=
                IMAGE_COLLECTOR
            {
                continue;
            }
            ensure!(
                collectors.len() < MAX_IMAGES,
                "definitions exceed {MAX_IMAGES} image collectors"
            );
            let path = root.join(view.text(usage, "sourcePath").await?);
            collectors.push(Arc::new(
                LocalImageFile::new(project, root, &path)
                    .await?
                    .with_asset(usage, view.node(usage, "asset").await?)?,
            ));
        }
        Ok(collectors)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn captions_survive_markdown_image_syntax() {
        let caption = "Figure [draft]: *A* \\ B_c";
        let source = format!("![{}](x)", super::Figures::escape(caption));
        let text: String = pulldown_cmark::Parser::new(&source)
            .filter_map(|event| {
                match event {
                    pulldown_cmark::Event::Text(text) => Some(text.to_string()),
                    _ => None,
                }
            })
            .collect();
        assert_eq!(text, caption);
    }
}
