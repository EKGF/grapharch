//! A local engine projection retained beside its exact semantic input.
use {
    super::{Diagram, plantuml_source::PlantUmlSource},
    crate::publication::{document::plantuml::LocalRenderer, image_assets},
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Projection {
    pub diagram: Diagram,
    pub source:  String,
    pub engine:  String,
    pub svg:     String,
}
impl Projection {
    pub async fn generate(
        diagram: Diagram,
        renderer: &LocalRenderer,
    ) -> Result<Self> {
        let mut source = PlantUmlSource::new(&diagram).render();
        let engine = renderer.version().await?;
        let mut svg = render(renderer, &source).await?;
        if !diagram.edges.is_empty() &&
            !super::layout::Layout::new(&diagram).has_hierarchy()
        {
            let alternative_source = source.replacen(
                "left to right direction",
                "top to bottom direction",
                1,
            );
            let alternative_svg = render(renderer, &alternative_source).await?;
            if layout_score(&alternative_svg)? > layout_score(&svg)? {
                source = alternative_source;
                svg = alternative_svg;
            }
        }
        ensure!(
            svg.len() <= 256 * 1024,
            "class diagram SVG exceeds 256 KiB"
        );
        Ok(Self { diagram, source, engine, svg })
    }

    pub fn parse(text: &str) -> Result<Self> {
        ensure!(
            text.len() <= 512 * 1024,
            "retained class diagram exceeds 512 KiB"
        );
        let value: Self = serde_json::from_str(text)?;
        Diagram::parse(&serde_json::to_string(&value.diagram)?)?;
        ensure!(
            value.source.len() <= 128 * 1024,
            "retained PlantUML source exceeds 128 KiB"
        );
        // Retained source is provenance, never an instruction to rerender.
        ensure!(
            value.engine.starts_with("PlantUML version") &&
                value.engine.len() <= 1024,
            "invalid retained class diagram engine identity"
        );
        ensure!(
            value.svg.len() <= 256 * 1024,
            "class diagram SVG exceeds 256 KiB"
        );
        image_assets::validate(value.svg.as_bytes(), "image/svg+xml")?;
        Ok(value)
    }
}

async fn render(renderer: &LocalRenderer, source: &str) -> Result<String> {
    let svg = String::from_utf8(renderer.svg(source).await?)?;
    ensure!(
        svg.len() <= 256 * 1024,
        "class diagram SVG exceeds 256 KiB"
    );
    Ok(svg)
}
fn layout_score(svg: &str) -> Result<f64> {
    let document = roxmltree::Document::parse(svg)?;
    let bounds = document
        .root_element()
        .attribute("viewBox")
        .ok_or_else(|| anyhow::anyhow!("PlantUML SVG has no viewBox"))?;
    let mut dimensions = bounds.split_whitespace().skip(2);
    let width: f64 = dimensions
        .next()
        .ok_or_else(|| anyhow::anyhow!("PlantUML SVG has no width"))?
        .parse()?;
    let height: f64 = dimensions
        .next()
        .ok_or_else(|| anyhow::anyhow!("PlantUML SVG has no height"))?
        .parse()?;
    ensure!(
        width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0,
        "invalid PlantUML SVG dimensions"
    );
    // Balance readable scaling with horizontal use, not narrowest fit.
    let scale = (900.0 / width).min(600.0 / height);
    Ok(scale * (width / height).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restoring_a_drawing_does_not_regenerate_its_source_with_new_layout_rules()
     {
        let retained = Projection {
            diagram: Diagram {
                focus:   super::super::Node {
                    id:       "urn:test:class".into(),
                    label:    "Class".into(),
                    kind:     super::super::NodeKind::Class,
                    link:     None,
                    imported: false,
                    datatype: false,
                },
                edges:   Vec::new(),
                shapes:  Vec::new(),
                limited: false,
            },
            source:  "@startuml\n' previous generated layout \
                      revision\nrectangle Class\n@enduml\n"
                .into(),
            engine:  "PlantUML version retained-fixture".into(),
            svg:     "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"120\" \
                      height=\"50\"><rect width=\"120\" height=\"50\"/></svg>"
                .into(),
        };
        let restored =
            Projection::parse(&serde_json::to_string(&retained).unwrap())
                .unwrap();
        assert_eq!(restored.source, retained.source);
        assert_eq!(restored.svg, retained.svg);
    }
    #[test]
    fn orientation_scoring_prefers_readable_landscape_over_narrow_portrait() {
        let landscape = "<svg xmlns=\"http://www.w3.org/2000/svg\" \
                         viewBox=\"0 0 800 400\"/>";
        let portrait = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 \
                        0 400 800\"/>";
        assert!(
            layout_score(landscape).unwrap() > layout_score(portrait).unwrap()
        );
    }
    #[test]
    fn a_small_font_scale_advantage_does_not_select_a_tall_portrait() {
        let landscape = "<svg xmlns=\"http://www.w3.org/2000/svg\" \
                         viewBox=\"0 0 950 268\"/>";
        let portrait = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 \
                        0 423 565\"/>";
        assert!(
            layout_score(landscape).unwrap() > layout_score(portrait).unwrap()
        );
    }
}
