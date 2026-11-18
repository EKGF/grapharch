//! Bounded facts that refine notation but are never drawn as raw edges.
use {
    super::{Edge, Node},
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

/// At most this many restriction facts on neighbouring classes.
pub(super) const RESTRICTION_FACTS: usize = 32;
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
    /// Complete simple restrictions on visible neighbouring classes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restrictions: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties:   Vec<PropertyUse>,
}
impl Context {
    pub fn is_empty(&self) -> bool {
        self.restrictions.is_empty() && self.properties.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.restrictions.len() <= RESTRICTION_FACTS &&
                self.properties.len() <= PROPERTY_USES &&
                self.properties.iter().all(|u| u.ranges.len() <= 4),
            "class diagram notation context exceeds its budget"
        );
        Ok(())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.restrictions
            .iter()
            .flat_map(|e| [&e.from, &e.to])
            .chain(self.properties.iter().flat_map(|u| u.ranges.iter()))
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.restrictions
            .iter_mut()
            .flat_map(|e| [&mut e.from, &mut e.to])
            .chain(self.properties.iter_mut().flat_map(|u| u.ranges.iter_mut()))
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
