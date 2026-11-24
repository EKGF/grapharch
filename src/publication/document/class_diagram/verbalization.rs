//! The accessible plain-language description of one drawn class diagram.
//!
//! Every element the PlantUML projection draws adds one sentence, so the
//! description states the same UML reading as the picture and never the
//! underlying RDF facts.
use {super::Node, std::fmt::Write};

/// Sentences describing a diagram, in drawing order.
#[derive(Default)]
pub(super) struct Verbalization {
    sentences: Vec<String>, // bounded: one per drawn element.
}
/// The qualifiers of one drawn member or association end.
pub(super) struct Qualifiers<'a> {
    pub multiplicity: Option<String>,
    pub tags:         &'a [String],
}
impl Qualifiers<'_> {
    fn phrase(&self) -> String {
        let mut text = String::new();
        if let Some(multiplicity) = &self.multiplicity {
            let _ = write!(text, ", multiplicity {multiplicity}");
        }
        for tag in self.tags {
            let _ = write!(text, ", {tag}");
        }
        text
    }
}
impl Verbalization {
    fn say(&mut self, sentence: String) { self.sentences.push(sentence); }

    pub fn class(&mut self, node: &Node, stereotype: Option<&str>) {
        self.say(match stereotype {
            Some(stereotype) => {
                format!("{} is a class, {stereotype}.", node.label)
            },
            None => format!("{} is a class.", node.label),
        });
    }

    pub fn element(&mut self, node: &Node) {
        self.say(format!("{} is a {}.", node.label, node.kind));
    }

    pub fn attribute(
        &mut self,
        class: &Node,
        name: &str,
        types: &str,
        qualifiers: &Qualifiers,
    ) {
        let typed = if types.is_empty() {
            String::new()
        } else {
            format!(" of type {types}")
        };
        self.say(format!(
            "{} has attribute {name}{typed}{}.",
            class.label,
            qualifiers.phrase()
        ));
    }

    pub fn constraint(&mut self, class: &Node, text: &str) {
        self.say(format!("{} has constraint {text}.", class.label));
    }

    pub fn literal(&mut self, class: &Node, literal: &Node) {
        self.say(format!(
            "{} has literal {}.",
            class.label, literal.label
        ));
    }

    pub fn association(
        &mut self,
        from: &Node,
        to: &Node,
        name: &str,
        qualifiers: &Qualifiers,
    ) {
        self.say(format!(
            "{} is associated with {} by {name}{}.",
            from.label,
            to.label,
            qualifiers.phrase()
        ));
    }

    pub fn ends(&mut self, from: &Node, to: &Node, forward: &str, back: &str) {
        self.say(format!(
            "{} is associated with {} by {forward}, and {} with {} by {back}.",
            from.label, to.label, to.label, from.label
        ));
    }

    pub fn placeholder(&mut self, to: &Node, name: &str) {
        self.say(format!(
            "Any resource is associated with {} by {name}.",
            to.label
        ));
    }

    pub fn generalization(
        &mut self,
        child: &Node,
        parent: &Node,
        tags: &[&str],
    ) {
        let tags = if tags.is_empty() {
            String::new()
        } else {
            format!(", {}", tags.join(", "))
        };
        self.say(format!(
            "{} is a kind of {}{tags}.",
            child.label, parent.label
        ));
    }

    pub fn disjoint(&mut self, first: &Node, second: &Node) {
        self.say(format!(
            "{} and {} are disjoint.",
            first.label, second.label
        ));
    }

    pub fn equivalent(&mut self, first: &Node, second: &Node) {
        self.say(format!(
            "{} and {} are equivalent.",
            first.label, second.label
        ));
    }

    pub fn note(&mut self, class: &Node, text: &str) {
        self.say(format!(
            "Constraint note on {}: {text}.",
            class.label
        ));
    }

    pub fn fact(&mut self, from: &Node, relation: &str, to: &Node) {
        self.say(format!("{} {relation} {}.", from.label, to.label));
    }

    /// The complete description of a diagram documenting `focus`.
    pub fn describe(&self, focus: &Node) -> String {
        let mut text = format!("UML class diagram of {}.", focus.label);
        for sentence in &self.sentences {
            text.push(' ');
            text.push_str(sentence);
        }
        text
    }
}
