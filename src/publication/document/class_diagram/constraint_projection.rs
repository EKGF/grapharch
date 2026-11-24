//! Ordinary UML qualifiers from unambiguous constraints; provenance stays
//! retained.
use {
    super::{
        Diagram,
        Node,
        constraints::Field,
        escape::Escaped,
        multiplicity::Cardinality,
        restriction::Prohibition,
        verbalization::{Qualifiers, Verbalization},
    },
    std::collections::BTreeMap,
};

pub(super) struct Property<'a> {
    pub property: &'a Node,
    pub range:    Option<&'a Node>,
    pub object:   bool,
    count:        Option<Cardinality>,
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
            (field.minimum.unwrap_or(0), field.maximum) ==
                (first.minimum.unwrap_or(0), first.maximum)
        });
        Self {
            property: &first.property,
            range:    same_type.then_some(first.range.as_ref()).flatten(),
            object:   same_type && first.object,
            count:    same_count.then(|| first.cardinality()).flatten(),
        }
    }

    fn endpoint(&self) -> String {
        self.count
            .and_then(Cardinality::multiplicity)
            .map(|m| format!("\"{m}\" "))
            .unwrap_or_default()
    }

    /// A maximum of zero values, drawn as a class constraint.
    pub fn prohibited(&self) -> bool {
        self.count == Some(Cardinality::Prohibited)
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
    constraints: &mut String,
    node: &Node,
    diagram: &Diagram,
    ids: &BTreeMap<&str, usize>,
    words: &mut Verbalization,
) {
    let shorthand = super::shorthand::Shorthand::new(diagram);
    for (_, field) in properties(diagram)
        .into_iter()
        .filter(|(class, _)| class.id == node.id)
    {
        if field.prohibited() {
            let prohibition = Prohibition::new(field.property, None);
            words.constraint(node, &prohibition.text());
            constraints.push_str(&prohibition.line(ids));
            continue;
        }
        if field.object {
            continue;
        }
        if field.range.is_none() &&
            shorthand.associations.iter().any(|association| {
                association.domain.id == node.id &&
                    association.node.id == field.property.id
            })
        {
            continue;
        }
        let multiplicity = field.count.and_then(Cardinality::multiplicity);
        words.attribute(
            node,
            &field.property.label,
            &field.range.map(|r| r.label.clone()).unwrap_or_default(),
            &Qualifiers {
                multiplicity: multiplicity.map(|m| m.to_string()),
                tags:         &[],
            },
        );
        let range = field
            .range
            .map(|r| format!(" : {}", Escaped::inline(&r.label)))
            .unwrap_or_default();
        let count = multiplicity.map(|m| format!(" [{m}]")).unwrap_or_default();
        output.push_str(&format!(
            "  {{field}} {}{range}{count} [[[./grapharch-node/n{}]]]\n",
            Escaped::inline(&field.property.label),
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
    words: &mut Verbalization,
    has_owl: impl Fn(&str, &Property<'_>) -> bool,
) {
    let layout = super::layout::Layout::new(diagram);
    for (class, field) in
        properties(diagram).into_iter().filter(|(_, f)| f.object)
    {
        let Some(range) = field.range else { continue };
        if field.prohibited() || has_owl(&class.id, &field) {
            continue;
        }
        words.association(class, range, &field.property.label, &Qualifiers {
            multiplicity: field
                .count
                .and_then(Cardinality::multiplicity)
                .map(|m| m.to_string()),
            tags:         &[],
        });
        output.push_str(&format!(
            "n{} {} {}n{} [[./grapharch-node/n{}]] : {}\n",
            ids[class.id.as_str()],
            layout.association_arrow(&class.id, &range.id),
            field.endpoint(),
            ids[range.id.as_str()],
            ids[field.property.id.as_str()],
            Escaped::wrapped(&field.property.label)
        ));
    }
}
