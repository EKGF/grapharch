//! Associations, generalizations, constraint lines and notes.
use {
    super::source::PlantUmlSource,
    crate::publication::document::class_diagram::{
        Node,
        constraint_projection,
        escape::Escaped,
        generalization::Generalizations,
        verbalization::Qualifiers,
    },
};
const EQUIVALENT: &str = "http://www.w3.org/2002/07/owl#equivalentClass";

impl PlantUmlSource<'_> {
    pub(super) fn placeholder(&mut self) {
        if self.shorthand.range_only.is_empty() {
            return;
        }
        self.output.push_str(&format!(
            "class \"{}\" as unknownDomain #white;line:gray;line.dotted\n",
            Escaped::inline("«any»")
        ));
        let mut lines = String::new();
        for property in &self.shorthand.range_only {
            self.words
                .placeholder(property.range, &property.property.label);
            lines.push_str(&format!(
                "unknownDomain ..> n{} [[./grapharch-node/n{}]] : {}\n",
                self.id(property.range),
                self.id(property.property),
                Escaped::wrapped(&property.property.label)
            ));
        }
        self.output.push_str(&lines);
    }

    pub(super) fn associations(&mut self) {
        let mut lines = String::new();
        let associations: Vec<_> = self
            .shorthand
            .associations
            .iter()
            .filter(|a| {
                !self.shorthand.supersedes(
                    &a.domain.id,
                    &a.node.id,
                    &a.range.id,
                )
            })
            .collect(); // bounded: the drawn associations.
        for association in &associations {
            let count = constraint_projection::association_count(
                self.diagram,
                &association.domain.id,
                &association.node.id,
                &association.range.id,
            );
            let multiplicity = Some(count.trim().trim_matches('"'))
                .filter(|m| !m.is_empty())
                .map(str::to_owned);
            let (from, to) = (association.domain, association.range);
            if let Some(inverse) =
                self.diagram.context.inverse(&association.node.id)
            {
                // The inverse's own association is the same line: draw it
                // once, from the property with the lower identity.
                let partner = associations.iter().any(|a| {
                    a.node.id == inverse.id &&
                        a.domain.id == to.id &&
                        a.range.id == from.id
                });
                if partner && inverse.id < association.node.id {
                    continue;
                }
                self.words.ends(
                    from,
                    to,
                    &association.node.label,
                    &inverse.label,
                );
                lines.push_str(&format!(
                    "n{} \"{}\" {} \"{}{}\" n{} [[./grapharch-node/n{}]]{}\n",
                    self.id(from),
                    Escaped::wrapped(&inverse.label),
                    self.layout.line(&from.id, &to.id, false),
                    Escaped::wrapped(&association.node.label),
                    multiplicity
                        .as_ref()
                        .map(|m| format!("\\n{m}"))
                        .unwrap_or_default(),
                    self.id(to),
                    self.id(association.node),
                    if association.alternative {
                        " : {xor}"
                    } else {
                        ""
                    },
                ));
                continue;
            }
            let tags = if association.alternative {
                vec!["xor".to_owned()]
            } else {
                Vec::new()
            };
            self.words.association(
                from,
                to,
                &association.node.label,
                &Qualifiers {
                    multiplicity: (!count.is_empty())
                        .then(|| count.trim().trim_matches('"').to_owned()),
                    tags:         &tags,
                },
            );
            lines.push_str(&format!(
                "n{} {} {count}n{} [[./grapharch-node/n{}]] : {}{}\n",
                self.id(from),
                self.layout.association_arrow(&from.id, &to.id),
                self.id(to),
                self.id(association.node),
                Escaped::wrapped(&association.node.label),
                if association.alternative {
                    "\\n{xor}"
                } else {
                    ""
                },
            ));
        }
        self.output.push_str(&lines);
        let associations = &self.shorthand.associations;
        constraint_projection::associations(
            &mut self.output,
            self.diagram,
            &self.identifiers,
            &mut self.words,
            |class, field| {
                associations.iter().any(|a| {
                    a.domain.id == class &&
                        a.node.id == field.property.id &&
                        field.range.is_some_and(|r| r.id == a.range.id)
                })
            },
        );
    }

    pub(super) fn restrictions(&mut self) {
        let mut lines = String::new();
        for (restriction, notation) in &self.shorthand.restrictions {
            if let Some(line) = notation.association(
                restriction.class,
                &self.identifiers,
                &self.layout,
            ) {
                notation.describe(restriction.class, &mut self.words);
                lines.push_str(&line);
            }
        }
        self.output.push_str(&lines);
    }

    pub(super) fn generalizations(&mut self) {
        let set = Generalizations::new(
            self.diagram,
            &self.shorthand.plan,
            &self.shorthand.consumed,
        );
        let mut lines = String::new();
        for arrow in &set.arrows {
            self.words
                .generalization(arrow.child, arrow.parent, &arrow.tags);
            lines.push_str(&format!(
                "n{} {} n{}{}\n",
                self.id(arrow.child),
                self.layout
                    .generalization_arrow(&arrow.child.id, &arrow.parent.id),
                self.id(arrow.parent),
                if arrow.tags.is_empty() {
                    String::new()
                } else {
                    format!(" : {{{}}}", arrow.tags.join(", "))
                }
            ));
        }
        for (first, second) in &set.disjoint {
            self.words.disjoint(first, second);
            lines.push_str(&self.constraint_line(first, second, "{disjoint}"));
        }
        self.output.push_str(&lines);
    }

    fn constraint_line(
        &self,
        first: &Node,
        second: &Node,
        text: &str,
    ) -> String {
        format!(
            "n{} {} n{} : {}\n",
            self.id(first),
            self.layout.line(&first.id, &second.id, true),
            self.id(second),
            Escaped::inline(text)
                .to_string()
                .replace("&#123;", "{")
                .replace("&#125;", "}")
        )
    }

    pub(super) fn remaining_facts(&mut self) {
        let drawn = Generalizations::new(
            self.diagram,
            &self.shorthand.plan,
            &self.shorthand.consumed,
        )
        .drawn;
        let mut lines = String::new();
        for (index, edge) in self.diagram.edges.iter().enumerate() {
            if self.shorthand.consumed.contains(&index) ||
                drawn.contains(&index)
            {
                continue;
            }
            if edge.predicate.as_deref() == Some(EQUIVALENT) &&
                edge.from.kind.is_class_like() &&
                edge.to.kind.is_class_like()
            {
                self.words.equivalent(&edge.from, &edge.to);
                lines.push_str(&self.constraint_line(
                    &edge.from,
                    &edge.to,
                    "«equivalent»",
                ));
                continue;
            }
            self.words.fact(&edge.from, &edge.relation, &edge.to);
            lines.push_str(&format!(
                "n{} {} n{} : {}\n",
                self.id(&edge.from),
                self.layout.relation_arrow(edge),
                self.id(&edge.to),
                Escaped::wrapped(&edge.relation)
            ));
        }
        self.output.push_str(&lines);
    }

    pub(super) fn notes(&mut self) {
        let mut lines = String::new();
        for (index, note) in self.shorthand.plan.notes.iter().enumerate() {
            self.words.note(note.class, &note.text);
            lines.push_str(&format!(
                "note \"{}\" as N{index}\nN{index} .. n{}\n",
                Escaped::paragraph(&note.text),
                self.id(note.class)
            ));
        }
        self.output.push_str(&lines);
    }
}
