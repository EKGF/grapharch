//! Ordinary UML qualifiers from unambiguous constraints; provenance stays
//! retained.
use {
    super::{
        Diagram,
        Node,
        constraints::Field,
        projection::{label, text},
    },
    std::collections::BTreeMap,
};

pub(super) struct Property<'a> {
    pub property: &'a Node,
    pub range:    Option<&'a Node>,
    pub object:   bool,
    count:        String,
}
impl<'a> Property<'a> {
    fn from_fields(fields: &[&'a Field]) -> Self {
        let first = fields[0];
        let same_type = fields.iter().all(|field| {
            field.object == first.object &&
                field.range.as_ref().map(|n| &n.id) ==
                    first.range.as_ref().map(|n| &n.id)
        });
        let same_count = fields.iter().all(|field| {
            (field.minimum, field.maximum) == (first.minimum, first.maximum)
        });
        Self {
            property: &first.property,
            range:    same_type.then_some(first.range.as_ref()).flatten(),
            object:   same_type && first.object,
            count:    if same_count {
                first.cardinality()
            } else {
                String::new()
            },
        }
    }

    fn endpoint(&self) -> String {
        if self.count.is_empty() {
            String::new()
        } else {
            format!("\"{}\" ", text(&self.count))
        }
    }
}
pub(super) fn properties(diagram: &Diagram) -> Vec<(&Node, Property<'_>)> {
    let mut groups = BTreeMap::<(&str, &str), (&Node, Vec<&Field>)>::new(); // bounded: 128 retained simple fields.
    for shape in &diagram.shapes {
        for field in &shape.fields {
            groups
                .entry((&shape.class.id, &field.property.id))
                .or_insert_with(|| (&shape.class, Vec::new()))
                .1
                .push(field);
        }
    }
    groups
        .into_values()
        .map(|(class, fields)| (class, Property::from_fields(&fields)))
        .collect() // bounded: at most 128 property views, not source facts.
}
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
    let shorthand = super::shorthand::Shorthand::new(diagram);
    for (_, field) in properties(diagram)
        .into_iter()
        .filter(|(class, f)| class.id == node.id && !f.object)
    {
        if field.range.is_none() &&
            shorthand.properties.iter().any(|property| {
                !property.attribute() &&
                    property.domain.id == node.id &&
                    property.node.id == field.property.id
            })
        {
            continue;
        }
        let range = field
            .range
            .map(|r| format!(" : {}", text(&r.label)))
            .unwrap_or_default();
        let count = if field.count.is_empty() {
            String::new()
        } else {
            format!(" [{}]", text(&field.count))
        };
        output.push_str(&format!(
            "  {{field}} {}{range}{count} [[[./grapharch-node/n{}]]]\n",
            text(&field.property.label),
            ids[field.property.id.as_str()]
        ));
    }
}
pub(super) fn association_count(
    diagram: &Diagram,
    class: &str,
    property: &str,
    range: &str,
) -> String {
    properties(diagram)
        .into_iter()
        .find(|(c, f)| {
            c.id == class &&
                f.property.id == property &&
                f.range.is_none_or(|r| r.id == range)
        })
        .map(|(_, f)| f.endpoint())
        .unwrap_or_default()
}
pub(super) fn associations(
    output: &mut String,
    diagram: &Diagram,
    ids: &BTreeMap<&str, usize>,
    has_owl: impl Fn(&str, &Property<'_>) -> bool,
) {
    let layout = super::layout::Layout::new(diagram);
    for (class, field) in
        properties(diagram).into_iter().filter(|(_, f)| f.object)
    {
        let Some(range) = field.range else { continue };
        if has_owl(&class.id, &field) {
            continue;
        }
        output.push_str(&format!(
            "n{} {} {}n{} [[./grapharch-node/n{}]] : {}\n",
            ids[class.id.as_str()],
            layout.association_arrow(&class.id, &range.id),
            field.endpoint(),
            ids[range.id.as_str()],
            ids[field.property.id.as_str()],
            label(&field.property.label)
        ));
    }
}
