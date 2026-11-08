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
    // Balance readable scaling with horizontal use of the content area. Pure
    // fit scoring can reward a tall, narrow drawing for a tiny font advantage.
    let scale = (900.0 / width).min(600.0 / height);
    Ok(scale * (width / height).min(1.0))
}

pub(super) fn nodes(diagram: &Diagram) -> BTreeMap<&str, &super::Node> {
    diagram
        .nodes()
        .map(|node| (node.id.as_str(), node))
        .collect() // bounded: at most thirty-seven nodes.
}
fn source(diagram: &Diagram) -> String {
    let nodes = nodes(diagram);
    let shorthand = super::shorthand::Shorthand::new(diagram);
    let layout = super::layout::Layout::new(diagram);
    let mut visible = shorthand.visible.clone();
    for (class, field) in super::constraint_projection::properties(diagram) {
        visible.insert(class.id.as_str());
        if field.object &&
            let Some(range) = field.range
        {
            visible.insert(range.id.as_str());
        }
    }
    let mut identifiers = BTreeMap::new(); // bounded: at most thirty-seven node IDs.
    let mut output =
        "@startuml\n!pragma layout smetana\nallowmixing\nleft to right \
         direction\nhide empty members\nhide circle\nskinparam shadowing \
         false\nskinparam backgroundColor transparent\nskinparam \
         defaultFontName Arial\nskinparam defaultFontSize 14\nskinparam \
         ArrowColor #68758A\nskinparam nodesep 30\nskinparam ranksep 45\n"
            .to_owned();
    identifiers
        .extend(nodes.keys().enumerate().map(|(index, id)| (*id, index)));
    if layout.has_hierarchy() {
        output = output.replacen(
            "left to right direction",
            "top to bottom direction",
            1,
        );
    }
    for (index, node) in nodes.values().enumerate() {
        if !visible.contains(node.id.as_str()) {
            continue;
        }
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
        if node.kind == "Class" {
            output.push_str(&format!(
                "class \"{label}\" as n{index} <<owl:Class>> \
                 [[./grapharch-node/n{index}]] {color} {{\n"
            ));
            for property in shorthand.properties.iter().filter(|p| {
                p.attribute() &&
                    p.domain.id == node.id &&
                    !super::constraint_projection::constrained(
                        diagram, &node.id, &p.node.id,
                    )
            }) {
                let property_index = nodes
                    .keys()
                    .position(|id| *id == property.node.id)
                    .expect("retained property node");
                let range = property
                    .range
                    .filter(|range| {
                        range.id != super::shorthand::GENERIC_RESOURCE
                    })
                    .map(|r| format!(" : {}", text(&r.label)))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "  {{field}} {}{range} \
                     [[[./grapharch-node/n{property_index}]]]\n",
                    text(&property.node.label)
                ));
            }
            super::constraint_projection::members(
                &mut output,
                node,
                diagram,
                &identifiers,
            );
            output.push_str("}\n");
        } else {
            output.push_str(&format!(
                "rectangle \"<<{}>>\\n{label}\" as n{index} \
                 [[./grapharch-node/n{index}]] {color}\n",
                text(&node.kind)
            ));
        }
    }
    if !shorthand.range_only.is_empty() {
        output.push_str(
            "class \"rdfs:Resource\" as unknownDomain \
             #white;line:gray;line.dotted\n",
        );
        for property in &shorthand.range_only {
            output.push_str(&format!(
                "unknownDomain ..> n{} [[./grapharch-node/n{}]] : {}\n",
                identifiers[property.range.id.as_str()],
                identifiers[property.property.id.as_str()],
                label(&property.property.label)
            ));
        }
    }
    for property in shorthand.properties.iter().filter(|p| !p.attribute()) {
        let range = property.range.expect("association has a declared range");
        output.push_str(&format!(
            "n{} {} {}n{} [[./grapharch-node/n{}]] : {}\n",
            identifiers[property.domain.id.as_str()],
            layout.association_arrow(&property.domain.id, &range.id),
            super::constraint_projection::association_count(
                diagram,
                &property.domain.id,
                &property.node.id,
                &range.id
            ),
            identifiers[range.id.as_str()],
            identifiers[property.node.id.as_str()],
            label(&property.node.label)
        ));
    }
    super::constraint_projection::associations(
        &mut output,
        diagram,
        &identifiers,
        |class, field| {
            shorthand.properties.iter().any(|p| {
                !p.attribute() &&
                    p.domain.id == class &&
                    p.node.id == field.property.id &&
                    p.range.map(|r| r.id.as_str()) ==
                        field.range.as_ref().map(|r| r.id.as_str())
            })
        },
    );
    for existential in &shorthand.existentials {
        output.push_str(&format!(
            "n{} {} n{} [[./grapharch-node/n{}]] : {}\\n{{at least one {}}}\n",
            identifiers[existential.class.id.as_str()],
            layout.association_arrow(
                &existential.class.id,
                &existential.filler.id
            ),
            identifiers[existential.filler.id.as_str()],
            identifiers[existential.property.id.as_str()],
            label(&existential.property.label),
            text(&existential.filler.label),
        ));
    }
    for (index, edge) in diagram.edges.iter().enumerate() {
        if shorthand.consumed.contains(&index) {
            continue;
        }
        let generalization = super::layout::is_generalization(edge);
        output.push_str(&format!(
            "n{} {} n{}{}\n",
            identifiers[edge.from.id.as_str()],
            if generalization {
                layout.generalization_arrow(edge)
            } else {
                layout.relation_arrow(edge)
            },
            identifiers[edge.to.id.as_str()],
            if generalization {
                String::new()
            } else {
                format!(" : {}", label(&edge.relation))
            }
        ));
    }
    output.push_str("@enduml\n");
    output
}
pub(super) fn label(value: &str) -> String {
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
pub(super) fn text(value: &str) -> String {
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
                focus:   super::super::Node {
                    id:    "urn:test:class".into(),
                    label: "Class".into(),
                    kind:  "Class".into(),
                    link:  None,
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
