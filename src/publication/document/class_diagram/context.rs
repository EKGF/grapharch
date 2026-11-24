//! Bounded facts that refine notation but are never drawn as raw edges.
use {
    super::{Node, expression::ClassAxiom},
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

/// At most this many restriction axioms on neighbouring classes.
pub(super) const NEIGHBOUR_AXIOMS: usize = 16;
/// At most this many restricted class-property pairs.
pub(super) const PROPERTY_USES: usize = 24;

/// How one class uses one restricted property, read from its declarations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PropertyUse {
    pub class:     String,
    pub property:  String,
    /// Named declared ranges of the property, in IRI order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ranges:    Vec<Node>, // bounded: four declared ranges.
    /// The class is not a declared domain, but an asserted superclass is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherited: bool,
}
/// Notation context of one diagram.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Context {
    /// Restriction axioms of visible neighbouring classes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axioms:     Vec<ClassAxiom>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<PropertyUse>,
    /// Declared inverses of the diagram's properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inverses:   Vec<Inverse>,
}
/// A property declared `owl:inverseOf` another.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inverse {
    pub property: String,
    pub inverse:  Node,
}
impl Context {
    pub fn is_empty(&self) -> bool {
        self.axioms.is_empty() &&
            self.properties.is_empty() &&
            self.inverses.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.axioms.len() <= NEIGHBOUR_AXIOMS &&
                self.axioms.iter().all(|a| a.nodes().len() <= 64) &&
                self.properties.len() <= PROPERTY_USES &&
                self.inverses.len() <= PROPERTY_USES &&
                self.properties.iter().all(|u| u.ranges.len() <= 4),
            "class diagram notation context exceeds its budget"
        );
        Ok(())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.axioms
            .iter()
            .flat_map(ClassAxiom::nodes)
            .chain(self.properties.iter().flat_map(|u| u.ranges.iter()))
            .chain(self.inverses.iter().map(|i| &i.inverse))
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.axioms
            .iter_mut()
            .flat_map(ClassAxiom::nodes_mut)
            .chain(self.properties.iter_mut().flat_map(|u| u.ranges.iter_mut()))
            .chain(self.inverses.iter_mut().map(|i| &mut i.inverse))
    }

    /// The declared inverse of a property, when one was read.
    pub fn inverse(&self, property: &str) -> Option<&Node> {
        self.inverses
            .iter()
            .find(|i| i.property == property)
            .map(|i| &i.inverse)
    }

    /// The declared use of a property by a class, when it was read.
    pub fn property_use(
        &self,
        class: &str,
        property: &str,
    ) -> Option<&PropertyUse> {
        self.properties
            .iter()
            .find(|u| u.class == class && u.property == property)
    }
}
