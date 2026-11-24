//! The bounded semantic input of one class diagram.
use {
    super::{NodeKind, constraints, context::Context, expression::ClassAxiom},
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

const DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
const RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
/// At most this many anonymous class axioms of the focus class.
pub(super) const AXIOMS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    pub id:       String,
    pub label:    String,
    pub kind:     NodeKind,
    pub link:     Option<String>,
    /// Declared by another ontology than the diagram's focus term.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub imported: bool,
    /// An RDF datatype rather than a class of individuals.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub datatype: bool,
    /// The vocabulary named under a box whose label another box shares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from:     Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Edge {
    pub from:      Node,
    pub relation:  String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    pub to:        Node,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Diagram {
    pub focus:   Node,
    pub edges:   Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes:  Vec<constraints::Shape>,
    /// Axioms relating the focus class to anonymous class expressions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axioms:  Vec<ClassAxiom>,
    #[serde(default)]
    pub limited: bool,
    #[serde(default, skip_serializing_if = "Context::is_empty")]
    pub context: Context,
}
impl Node {
    /// A literal value endpoint, identified by its typed RDF form.
    pub fn literal(value: &oxrdf::Literal) -> Node {
        Node {
            id:       value.to_string(),
            label:    value.value().into(),
            kind:     NodeKind::LiteralValue,
            link:     None,
            imported: false,
            datatype: false,
            from:     None,
        }
    }
}
impl Node {
    /// A non-negative OWL count, parsed from the original typed RDF value
    /// and never from its presentation label.
    pub fn count(&self) -> Option<u64> {
        if self.kind != NodeKind::LiteralValue {
            return None;
        }
        let literal = self.id.parse::<oxrdf::Literal>().ok()?;
        if !matches!(
            literal.datatype().as_str(),
            "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" |
                "http://www.w3.org/2001/XMLSchema#integer"
        ) {
            return None;
        }
        literal.value().parse().ok()
    }

    /// The UML metaclass this resource names, if any.
    pub(super) fn metaclass(&self) -> Option<super::metaclass::Metaclass> {
        super::metaclass::Metaclass::of(&self.id)
    }

    /// The name of a datatype or class used as a member type.
    pub fn type_label(&self) -> String {
        if let Some(metaclass) = self.metaclass() {
            return metaclass.name().to_owned();
        }
        self.id
            .strip_prefix("http://www.w3.org/2001/XMLSchema#")
            .map(|name| format!("xsd:{name}"))
            .unwrap_or_else(|| self.label.clone())
    }
}
impl Edge {
    pub fn declares_domain(&self) -> bool {
        self.predicate.as_deref() == Some(DOMAIN)
    }

    pub fn declares_range(&self) -> bool {
        self.predicate.as_deref() == Some(RANGE)
    }

    pub fn declares_union(&self) -> bool {
        self.predicate.as_deref() == Some(super::union_domain::UNION)
    }
}
impl Diagram {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 256 * 1024,
            "class diagram exceeds 256 KiB"
        );
        let diagram: Self = serde_json::from_str(source)?;
        ensure!(
            diagram.edges.len() <= 18,
            "class diagram exceeds eighteen relationship budget"
        );
        ensure!(
            diagram.shapes.len() <= 32 &&
                diagram.shapes.iter().map(|s| s.fields.len()).sum::<usize>() <=
                    128,
            "class diagram shape budget exceeded"
        );
        diagram.context.validate()?;
        ensure!(
            diagram.axioms.len() <= AXIOMS &&
                diagram.axioms.iter().all(|a| a.nodes().len() <= 64),
            "class diagram axiom budget exceeded"
        );
        let mut identities = std::collections::BTreeMap::new(); // bounded: at most thirty-seven nodes.
        for node in diagram.nodes() {
            if let Some(previous) = identities.insert(&node.id, node) {
                ensure!(
                    previous == node,
                    "conflicting class diagram node identity"
                );
            }
            ensure!(
                node.label.len() <= 512 && node.id.len() <= 2048,
                "class diagram node exceeds text budget"
            );
        }
        for edge in &diagram.edges {
            if let Some(predicate) = &edge.predicate {
                ensure!(
                    predicate.len() <= 2048,
                    "diagram predicate exceeds IRI budget"
                );
                oxrdf::NamedNode::new(predicate.clone())?;
            }
            ensure!(
                edge.relation.len() <= 256,
                "class diagram relation exceeds text budget"
            );
        }
        Ok(diagram)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        std::iter::once(&self.focus)
            .chain(self.edges.iter().flat_map(|e| [&e.from, &e.to]))
            .chain(self.shapes.iter().flat_map(|s| s.nodes()))
            .chain(self.axioms.iter().flat_map(ClassAxiom::nodes))
            .chain(self.context.nodes())
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        std::iter::once(&mut self.focus)
            .chain(self.edges.iter_mut().flat_map(|e| [&mut e.from, &mut e.to]))
            .chain(self.shapes.iter_mut().flat_map(|s| s.nodes_mut()))
            .chain(self.axioms.iter_mut().flat_map(ClassAxiom::nodes_mut))
            .chain(self.context.nodes_mut())
    }

    /// Number each anonymous expression box in order of first appearance.
    pub fn number_expressions(&mut self) {
        let mut ordinals = std::collections::BTreeMap::new(); // bounded: thirty-seven nodes.
        for node in self.nodes() {
            if node.kind.is_expression() {
                let next = ordinals.len() + 1;
                ordinals.entry(node.id.clone()).or_insert(next);
            }
        }
        for node in self.nodes_mut() {
            if let Some(ordinal) = ordinals.get(&node.id) {
                node.label = format!("Anonymous OWL expression {ordinal}");
            }
        }
    }
}
