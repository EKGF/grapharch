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
        let source = source(&diagram);
        let engine = renderer.version().await?;
        let svg = String::from_utf8(renderer.svg(&source).await?)?;
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
            value.source == source(&value.diagram),
            "class diagram source does not match its retained assertions"
        );
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
        let label = node
            .label
            .split_once(':')
            .map(|(prefix, local)| {
                format!("{}:\\n{}", text(prefix), text(local))
            })
            .unwrap_or_else(|| text(&node.label));
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
            text(&edge.relation)
        ));
    }
    output.push_str("@enduml\n");
    output
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
