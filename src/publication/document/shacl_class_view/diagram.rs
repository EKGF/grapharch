//! A selected shape's declared validation fields, separate from OWL semantics.
use {
    super::{Property, SH, Shape},
    crate::publication::{document::plantuml::LocalRenderer, image_assets},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};
pub(crate) const LANGUAGE: &str = "grapharch-shacl-view";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Field {
    pub property_shape: String,
    pub path:           String,
    pub constraints:    Vec<(String, String)>, /* bounded: sixty-four
                                                * original scalar facts. */
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Projection {
    pub class:   String,
    pub shape:   String,
    pub label:   String,
    pub fields:  Vec<Field>, /* bounded: thirty-two property shapes;
                              * repeated paths stay distinct. */
    pub omitted: usize,
    pub source:  String,
    pub engine:  String,
    pub svg:     String,
}
impl Projection {
    pub(crate) fn parse(body: &str) -> Result<Self> {
        ensure!(
            body.len() <= 512 * 1024,
            "SHACL diagram exceeds 512 KiB"
        );
        let value: Self = serde_json::from_str(body)?;
        ensure!(
            value.fields.len() <= 32 &&
                value.source.len() <= 128 * 1024 &&
                value.svg.len() <= 256 * 1024 &&
                value.label.len() <= 4096,
            "SHACL diagram exceeds projection budget"
        );
        NamedNode::new(value.class.clone())?;
        NamedNode::new(value.shape.clone())?;
        ensure!(
            value.engine.starts_with("PlantUML version ") &&
                value.engine.len() <= 1024,
            "invalid SHACL diagram engine identity"
        );
        image_assets::validate(value.svg.as_bytes(), "image/svg+xml")?;
        Ok(value)
    }
}
pub(super) async fn generate(
    shape: &Shape,
    class: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    renderer: &LocalRenderer,
) -> Result<Projection> {
    let mut fields = Vec::new(); // bounded: thirty-two properties; only complete simple constraint sets projected.
    for property in &shape.properties {
        let Some(path) = simple(property) else {
            continue;
        };
        let constraints = property
            .facts
            .iter()
            .filter(|(p, _)| {
                matches!(
                    p.as_str().strip_prefix(SH),
                    Some(
                        "minCount" |
                            "maxCount" |
                            "datatype" |
                            "class" |
                            "nodeKind"
                    )
                )
            })
            .map(|(p, v)| (p.as_str().to_owned(), v.to_string()))
            .collect();
        fields.push(Field {
            property_shape: property.id.as_str().into(),
            path: path.as_str().into(),
            constraints,
        });
    }
    let label = format!(
        "{}\nValidation shape: {}",
        short(class.as_str(), prefixes),
        short(shape.id.as_str(), prefixes)
    );
    let mut source = format!(
        "@startuml\n!pragma layout smetana\n!pragma svek_trace \
         false\nskinparam backgroundColor white\nskinparam shadowing \
         false\nskinparam classAttributeIconSize 0\nskinparam defaultFontName \
         SansSerif\nskinparam defaultFontSize 14\nclass \"{}\" as validation \
         <<SHACL NodeShape>> {{\n",
        escape(&label)
    );
    for field in &fields {
        let mut lines = vec![short(&field.path, prefixes)]; // bounded: sixty-five field lines.
        for (predicate, raw) in &field.constraints {
            let term: Term = raw.parse()?;
            let value = match term {
                Term::NamedNode(node) => short(node.as_str(), prefixes),
                Term::Literal(value) => value.value().into(),
                _ => unreachable!("simple scalar constraints"),
            };
            lines.push(format!(
                "{} = {value}",
                predicate.strip_prefix(SH).unwrap_or(predicate)
            ));
        }
        source.push_str(&format!(
            "  {{field}} {}\n",
            escape(&lines.join("\n"))
        ));
        source.push_str("  --\n");
    }
    let omitted = shape.properties.len() - fields.len();
    if fields.is_empty() {
        source.push_str("  {field} See full shape for property constraints\n");
    }
    if omitted > 0 {
        source.push_str(&format!(
            "  {{field}} {omitted} complex property shapes: see full \
             definition\n"
        ));
    }
    source.push_str("}\n@enduml\n");
    ensure!(
        source.len() <= 128 * 1024,
        "SHACL PlantUML source exceeds 128 KiB"
    );
    let engine = renderer.version().await?;
    let svg = String::from_utf8(renderer.svg(&source).await?)?;
    ensure!(
        svg.len() <= 256 * 1024,
        "SHACL SVG exceeds 256 KiB"
    );
    Ok(Projection {
        class: class.as_str().into(),
        shape: shape.id.as_str().into(),
        label,
        fields,
        omitted,
        source,
        engine,
        svg,
    })
}
fn simple(property: &Property) -> Option<&NamedNode> {
    let [Term::NamedNode(path)] = property.paths.as_slice() else {
        return None;
    };
    if path.as_str().starts_with("urn:grapharch:skolem:") {
        return None;
    }
    // Qualified/logical constraints remain in the exact linked source/table;
    // never show their partial projection as an ordinary multiplicity.
    if property.facts.iter().any(|(p, _)| {
        matches!(
            p.as_str().strip_prefix(SH),
            Some(
                "or" | "and" |
                    "not" |
                    "xone" |
                    "qualifiedValueShape" |
                    "qualifiedMinCount" |
                    "qualifiedMaxCount" |
                    "node"
            )
        )
    }) {
        return None;
    }
    Some(path)
}
fn short(iri: &str, prefixes: &BTreeMap<String, String>) -> String {
    prefixes
        .iter()
        .filter(|(base, _)| iri.starts_with(base.as_str()))
        .max_by_key(|(base, _)| base.len())
        .map(|(base, prefix)| format!("{prefix}:{}", &iri[base.len()..]))
        .unwrap_or_else(|| iri.rsplit(['#', '/']).next().unwrap_or(iri).into())
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('{', "&#123;")
        .replace('}', "&#125;")
        .replace('\\', "&#92;")
        .replace('\n', "\\n")
        .replace('\r', "")
}
