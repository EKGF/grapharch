//! An optional profile switch, which must be an `xsd:boolean` literal.
use {
    crate::publication::render::model::Plan,
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};

const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

pub(super) struct Flag<'a> {
    property: &'a str,
    default:  bool,
}

impl<'a> Flag<'a> {
    pub const fn new(property: &'a str, default: bool) -> Self {
        Self { property, default }
    }

    pub async fn read(&self, plan: &Plan, subject: &NamedNode) -> Result<bool> {
        match plan.source.optional(subject, self.property).await? {
            None => Ok(self.default),
            Some(Term::Literal(value))
                if value.datatype().as_str() == XSD_BOOLEAN =>
            {
                match value.value() {
                    "true" | "1" => Ok(true),
                    "false" | "0" => Ok(false),
                    other => {
                        bail!("{subject} invalid {} {other}", self.property)
                    },
                }
            },
            Some(_) => {
                bail!(
                    "{subject} {} requires an xsd:boolean literal",
                    self.property
                )
            },
        }
    }
}
