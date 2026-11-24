//! Deterministic PlantUML lines for one restriction notation.
use {
    super::notation::Notation,
    crate::publication::document::class_diagram::{
        Node,
        escape::Escaped,
        layout::Layout,
        verbalization::{Qualifiers, Verbalization},
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

    /// Add the sentence describing this notation on `class`.
    pub fn describe(&self, class: &Node, words: &mut Verbalization) {
        match self {
            Self::Hidden => {},
            Self::Member { name, types, multiplicity, tags, .. } => {
                let types: Vec<String> =
                    types.iter().map(|t| t.type_label()).collect(); // bounded: four ranges.
                words.attribute(
                    class,
                    name,
                    &types.join(" and "),
                    &Qualifiers {
                        multiplicity: multiplicity.map(|m| m.to_string()),
                        tags,
                    },
                );
            },
            Self::Association {
                property,
                target,
                multiplicity,
                tags,
                reversed,
            } => {
                let qualifiers = Qualifiers {
                    multiplicity: multiplicity.map(|m| m.to_string()),
                    tags,
                };
                if *reversed {
                    words.association(
                        target,
                        class,
                        &property.label,
                        &qualifiers,
                    );
                } else {
                    words.association(
                        class,
                        target,
                        &property.label,
                        &qualifiers,
                    );
                }
            },
            Self::Prohibition { property, filler } => {
                words.constraint(
                    class,
                    &Prohibition::new(property, *filler).text(),
                );
            },
        }
    }

    /// A class-compartment attribute row.
    pub fn member(&self, ids: &BTreeMap<&str, usize>) -> Option<String> {
        let Self::Member {
            name,
            property,
            types,
            multiplicity,
            tags: qualifiers,
        } = self
        else {
            return None;
        };
        let names: Vec<String> = types.iter().map(|t| t.type_label()).collect(); // bounded: four ranges.
        let mut line = format!("  {{field}} {}", Escaped::inline(name));
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
            reversed,
        } = self
        else {
            return None;
        };
        let multiplicity = multiplicity
            .map(|m| format!("\"{m}\" "))
            .unwrap_or_default();
        let label = format!(
            "{}{}",
            Escaped::wrapped(&property.label),
            if qualifiers.is_empty() {
                String::new()
            } else {
                format!("\\n{}", tags(qualifiers))
            },
        );
        let link = ids[property.id.as_str()];
        Some(
            if *reversed {
                // Each value of the class is reached from 1..* filler values:
                // the count belongs at the filler's end of the association.
                format!(
                    "n{} {multiplicity}{} n{} [[./grapharch-node/n{link}]] : \
                     {label}\n",
                    ids[target.id.as_str()],
                    layout.association_arrow(&target.id, &class.id),
                    ids[class.id.as_str()],
                )
            } else {
                format!(
                    "n{} {} {multiplicity}n{} [[./grapharch-node/n{link}]] : \
                     {label}\n",
                    ids[class.id.as_str()],
                    layout.association_arrow(&class.id, &target.id),
                    ids[target.id.as_str()],
                )
            },
        )
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

    /// The constraint text, `no p` or `no p in C`.
    pub fn text(&self) -> String {
        match self.filler {
            Some(filler) => {
                format!(
                    "no {} in {}",
                    self.property.label,
                    filler.type_label()
                )
            },
            None => format!("no {}", self.property.label),
        }
    }

    /// `{no p}` or `{no p in C}` as a class constraint row after the
    /// attributes.
    pub fn line(&self, ids: &BTreeMap<&str, usize>) -> String {
        format!(
            "  {{field}} {{{}}} [[[./grapharch-node/n{}]]]\n",
            Escaped::inline(&self.text()),
            ids[self.property.id.as_str()]
        )
    }
}
