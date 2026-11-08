//! SHACL annotations in the same class drawing, with exact shape scopes.
use {
    super::{
        Diagram,
        Node,
        constraints::Field,
        projection::{label, text},
    },
    std::collections::BTreeMap,
};

pub(super) fn constrained(
    diagram: &Diagram,
    class: &str,
    property: &str,
) -> bool {
    diagram.shapes.iter().any(|s| {
        s.class.id == class &&
            s.fields.iter().any(|f| f.property.id == property)
    })
}
pub(super) fn members(
    output: &mut String,
    node: &Node,
    diagram: &Diagram,
    ids: &BTreeMap<&str, usize>,
) {
    for shape in diagram.shapes.iter().filter(|s| s.class.id == node.id) {
        output.push_str(&format!(
            "  --\n  {{field}} SHACL {} [[[./grapharch-node/n{}]]]\n",
            if shape.source.label.len() <= 32 {
                text(&shape.source.label)
            } else {
                label(&shape.source.label)
            },
            ids[shape.source.id.as_str()]
        ));
        for field in shape.fields.iter().filter(|f| !f.object) {
            let range = field
                .range
                .as_ref()
                .map(|r| format!(" : {}", text(&r.label)))
                .unwrap_or_default();
            let cardinality = field.cardinality();
            let counts = if cardinality.is_empty() {
                String::new()
            } else {
                format!(" [{}]", text(&cardinality))
            };
            output.push_str(&format!(
                "  {{field}} {}{range}{counts} [[[./grapharch-node/n{}]]]\n",
                text(&field.property.label),
                ids[field.property.id.as_str()]
            ));
        }
        if shape.omitted > 0 {
            output.push_str(&format!(
                "  {{field}} {} additional constraints: see shape\n",
                shape.omitted
            ));
        }
    }
}
pub(super) fn association_annotations(
    diagram: &Diagram,
    class: &str,
    property: &str,
    range: &str,
) -> String {
    diagram
        .shapes
        .iter()
        .filter(|s| s.class.id == class)
        .flat_map(|s| {
            s.fields
                .iter()
                .filter(move |f| {
                    f.object &&
                        f.property.id == property &&
                        f.range.as_ref().is_some_and(|r| r.id == range)
                })
                .map(move |f| {
                    format!(
                        "\\nSHACL {} {}",
                        label(&s.source.label),
                        text(&f.cardinality())
                    )
                })
        })
        .collect::<Vec<_>>()
        .join("") // bounded: thirty-two shape scopes, 128 fields.
}
pub(super) fn associations(
    output: &mut String,
    diagram: &Diagram,
    ids: &BTreeMap<&str, usize>,
    has_owl: impl Fn(&str, &Field) -> bool,
) {
    for shape in &diagram.shapes {
        for field in shape.fields.iter().filter(|f| f.object) {
            let Some(range) = &field.range else { continue };
            if has_owl(&shape.class.id, field) {
                continue;
            }
            let count = field.cardinality();
            let count = if count.is_empty() {
                String::new()
            } else {
                format!("\"{}\" ", text(&count))
            };
            output.push_str(&format!(
                "n{} -[#3B6E8F,dashed]-> {count}n{} [[./grapharch-node/n{}]] \
                 : {}\\nSHACL {}\n",
                ids[shape.class.id.as_str()],
                ids[range.id.as_str()],
                ids[field.property.id.as_str()],
                label(&field.property.label),
                label(&shape.source.label)
            ));
        }
    }
}
