//! Subject identity attached to an RDF predicate table, independent of output.
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};
pub(crate) const LANGUAGE: &str = "grapharch-table-subject";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Subject {
    pub identifier: String,
    pub iri:        String,
}
impl Subject {
    pub fn new(identifier: String, iri: String) -> Result<Self> {
        ensure!(
            !identifier.is_empty() && identifier.len() <= 4096,
            "invalid table subject identifier"
        );
        ensure!(
            iri.len() <= 4096,
            "table subject IRI exceeds budget"
        );
        oxrdf::NamedNode::new(iri.clone())?;
        Ok(Self { identifier, iri })
    }

    pub fn markdown(&self) -> Result<String> {
        Ok(format!(
            "```{LANGUAGE}\n{}\n```\n\n",
            serde_json::to_string(self)?
        ))
    }

    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 16384,
            "table subject metadata exceeds budget"
        );
        let value: Self = serde_json::from_str(source)?;
        Self::new(value.identifier, value.iri)
    }
}
