//! A local engine projection retained beside its exact semantic input.
use {
    super::Diagram,
    crate::publication::{document::plantuml::LocalRenderer, image_assets},
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
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
        let mut source = source(&diagram);
        let engine = renderer.version().await?;
        let mut svg = render(renderer, &source).await?;
        if width(&svg)? > 600.0 {
            let alternative_source = source.replacen(
                "left to right direction",
                "top to bottom direction",
                1,
            );
            let alternative_svg = render(renderer, &alternative_source).await?;
            if width(&alternative_svg)? < width(&svg)? {
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

    pub(super) fn width(&self) -> Result<f64> { width(&self.svg) }

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
        // Source is provenance, not an instruction to render again. A newer
        // generator may use different formatting/layout rules without changing
        // this pinned drawing.
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
fn width(svg: &str) -> Result<f64> {
    let document = roxmltree::Document::parse(svg)?;
    let bounds = document
        .root_element()
        .attribute("viewBox")
        .ok_or_else(|| anyhow::anyhow!("PlantUML SVG has no viewBox"))?;
    let width: f64 = bounds
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("PlantUML SVG has no viewBox width"))?
        .parse()?;
    ensure!(
        width.is_finite() && width > 0.0,
        "invalid PlantUML SVG width"
    );
    Ok(width)
}

pub(super) fn nodes(diagram: &Diagram) -> BTreeMap<&str, &super::Node> {
    diagram
        .nodes()
        .map(|node| (node.id.as_str(), node))
        .collect() // bounded: at most thirty-seven nodes.
}
fn source(diagram: &Diagram) -> String {
    let nodes = nodes(diagram);
    let mut identifiers = BTreeMap::new(); // bounded: at most thirty-seven node IDs.
    let mut output = "@startuml\n!pragma layout smetana\nleft to right \
                      direction\nhide empty members\nhide circle\nskinparam \
                      shadowing false\nskinparam backgroundColor \
                      transparent\nskinparam defaultFontName Arial\nskinparam \
                      defaultFontSize 14\nskinparam ArrowColor \
                      #68758A\nskinparam nodesep 30\nskinparam ranksep 45\n"
        .to_owned();
    for (index, node) in nodes.values().enumerate() {
        identifiers.insert(node.id.as_str(), index);
        let color = if node.id == diagram.focus.id {
            "#E9EDFF"
        } else if node.kind.contains("property") {
            "#FFF5E9"
        } else {
            "#FFFFFF"
        };
        let label = label(&node.label);
        // Links are opaque node references. Publication routes are resolved
        // after layout; source-supplied links never enter PlantUML syntax.
        output.push_str(&format!(
            "rectangle \"<<{}>>\\n{label}\" as n{index} \
             [[./grapharch-node/n{index}]] {color}\n",
            text(&node.kind)
        ));
    }
    for edge in &diagram.edges {
        output.push_str(&format!(
            "n{} --> n{} : {}\n",
            identifiers[edge.from.id.as_str()],
            identifiers[edge.to.id.as_str()],
            label(&edge.relation)
        ));
    }
    output.push_str("@enduml\n");
    output
}
fn label(value: &str) -> String {
    if let Some((prefix, local)) = value.split_once(':') {
        return format!("{}:\\n{}", text(prefix), label(local));
    }
    let mut lines = Vec::new(); // bounded: source labels are capped at 512 bytes.
    let mut line = String::new();
    let mut length = 0;
    for character in value.chars() {
        if length == 24 || (length >= 10 && character.is_uppercase()) {
            lines.push(text(&line));
            line.clear();
            length = 0;
        }
        line.push(character);
        length += 1;
    }
    lines.push(text(&line));
    lines.join("\\n")
}
fn text(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() ||
            matches!(character, ' ' | ':' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push_str(&format!("<U+{:04X}>", character as u32));
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restoring_a_drawing_does_not_regenerate_its_source_with_new_layout_rules()
     {
        let retained = Projection {
            diagram: Diagram {
                focus: super::super::Node {
                    id:    "urn:test:class".into(),
                    label: "Class".into(),
                    kind:  "Class".into(),
                    link:  None,
                },
                edges: Vec::new(),
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
    fn long_labels_wrap_without_discarding_identifier_characters() {
        let wrapped = label("InformationSensitivityClassification");
        assert!(wrapped.contains("\\n"));
        assert_eq!(
            wrapped.replace("\\n", ""),
            "InformationSensitivityClassification"
        );
        assert_eq!(label("rdfs:subClassOf"), "rdfs:\\nsubClassOf");
    }
    #[test]
    fn source_text_cannot_inject_plantuml_or_creole_directives() {
        let malicious =
            "\"\n@enduml\n!include https://evil.example/a\n<img:x>[[x]]\\n";
        let encoded = text(malicious);
        assert!(!encoded.contains('\n'));
        assert!(!encoded.contains('"'));
        assert!(!encoded.contains("!include"));
        assert!(!encoded.contains("<img:"));
        assert!(!encoded.contains("[["));
    }
}
