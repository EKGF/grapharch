//! Deterministic PlantUML lines for one restriction notation.
use {
    super::notation::Notation,
    crate::publication::document::class_diagram::{
        Node,
        escape::Escaped,
        layout::Layout,
    },
    std::collections::BTreeMap,
};

fn tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let escaped: Vec<String> = tags
        .iter()
        .map(|tag| Escaped::inline(tag).to_string())
        .collect(); // bounded: three qualifiers.
    format!("{{{}}}", escaped.join(", "))
}
impl<'a> Notation<'a> {
    /// A diagram box this notation needs beside the restricted class.
    pub fn target(&self) -> Option<&'a Node> {
        match self {
            Self::Association { target, .. } => Some(*target),
            _ => None,
        }
    }

    /// A class-compartment attribute row.
    pub fn member(&self, ids: &BTreeMap<&str, usize>) -> Option<String> {
        let Self::Member { property, types, multiplicity, tags: qualifiers } =
            self
        else {
            return None;
        };
        let names: Vec<String> = types.iter().map(|t| t.type_label()).collect(); // bounded: four ranges.
        let mut line =
            format!("  {{field}} {}", Escaped::inline(&property.label));
        if !names.is_empty() {
            line.push_str(&format!(
                " : {}",
                Escaped::inline(&names.join(" and "))
            ));
        }
        if let Some(multiplicity) = multiplicity {
            line.push_str(&format!(" [{multiplicity}]"));
        }
        if !qualifiers.is_empty() {
            line.push_str(&format!(" {}", tags(qualifiers)));
        }
        line.push_str(&format!(
            " [[[./grapharch-node/n{}]]]\n",
            ids[property.id.as_str()]
        ));
        Some(line)
    }

    /// A class-constraint row stating a prohibition.
    pub fn constraint(&self, ids: &BTreeMap<&str, usize>) -> Option<String> {
        let Self::Prohibition { property, filler } = self else {
            return None;
        };
        Some(Prohibition::new(property, *filler).line(ids))
    }

    /// An association from the restricted class.
    pub fn association(
        &self,
        class: &Node,
        ids: &BTreeMap<&str, usize>,
        layout: &Layout<'_>,
    ) -> Option<String> {
        let Self::Association {
            property,
            target,
            multiplicity,
            tags: qualifiers,
        } = self
        else {
            return None;
        };
        Some(format!(
            "n{} {} {}n{} [[./grapharch-node/n{}]] : {}{}\n",
            ids[class.id.as_str()],
            layout.association_arrow(&class.id, &target.id),
            multiplicity
                .map(|m| format!("\"{m}\" "))
                .unwrap_or_default(),
            ids[target.id.as_str()],
            ids[property.id.as_str()],
            Escaped::wrapped(&property.label),
            if qualifiers.is_empty() {
                String::new()
            } else {
                format!("\\n{}", tags(qualifiers))
            },
        ))
    }
}
/// A prohibited property, optionally only within a named filler.
pub(in super::super) struct Prohibition<'a> {
    property: &'a Node,
    filler:   Option<&'a Node>,
}
impl<'a> Prohibition<'a> {
    pub fn new(property: &'a Node, filler: Option<&'a Node>) -> Self {
        Self { property, filler }
    }

    /// `{no p}` or `{no p in C}` as a class constraint row after the
    /// attributes.
    pub fn line(&self, ids: &BTreeMap<&str, usize>) -> String {
        let text = match self.filler {
            Some(filler) => {
                format!(
                    "no {} in {}",
                    self.property.label,
                    filler.type_label()
                )
            },
            None => format!("no {}", self.property.label),
        };
        format!(
            "  {{field}} {{{}}} [[[./grapharch-node/n{}]]]\n",
            Escaped::inline(&text),
            ids[self.property.id.as_str()]
        )
    }
}
