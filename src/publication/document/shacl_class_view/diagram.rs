//! A selected shape's declared validation fields, separate from OWL semantics.
use {
    crate::publication::image_assets,
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    serde::{Deserialize, Serialize},
};
pub(crate) const LANGUAGE: &str = "grapharch-shacl-view";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Field {
    pub property_shape: String,
    pub path:           String,
    pub constraints:    Vec<(String, String)>, /* bounded: sixty-four
                                                * original scalar facts. */
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Projection {
    pub class:   String,
    pub shape:   String,
    pub label:   String,
    pub fields:  Vec<Field>, /* bounded: thirty-two property shapes;
                              * repeated paths stay distinct. */
    pub omitted: usize,
    pub source:  String,
    pub engine:  String,
    pub svg:     String,
}
impl Projection {
    pub(crate) fn parse(body: &str) -> Result<Self> {
        ensure!(
            body.len() <= 512 * 1024,
            "SHACL diagram exceeds 512 KiB"
        );
        let value: Self = serde_json::from_str(body)?;
        ensure!(
            value.fields.len() <= 32 &&
                value.source.len() <= 128 * 1024 &&
                value.svg.len() <= 256 * 1024 &&
                value.label.len() <= 4096,
            "SHACL diagram exceeds projection budget"
        );
        NamedNode::new(value.class.clone())?;
        NamedNode::new(value.shape.clone())?;
        ensure!(
            value.engine.starts_with("PlantUML version ") &&
                value.engine.len() <= 1024,
            "invalid SHACL diagram engine identity"
        );
        image_assets::validate(value.svg.as_bytes(), "image/svg+xml")?;
        Ok(value)
    }
}
