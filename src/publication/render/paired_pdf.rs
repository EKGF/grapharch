//! A specification page attaches a PDF compiled from its own content unless
//! its target sets `ga:pairedPdf false`, as a family does whose named PDF
//! edition targets already publish that document.
use {
    crate::publication::{Renderer, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

const PROPERTY: &str = "pairedPdf";
const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// Whether a target's renderer compiles a PDF beside its own output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PairedPdf {
    /// The target's output is a PDF (Typst) or carries one (specification).
    Compiled,
    /// The target publishes no PDF of its own.
    Omitted,
}

impl PairedPdf {
    pub(super) async fn read(
        view: &RevisionView,
        target: &NamedNode,
        renderer: Renderer,
    ) -> Result<Self> {
        let declared = Self::declared(view, target).await?;
        match renderer {
            Renderer::Typst => {
                ensure!(
                    declared.is_none(),
                    "{target} ga:{PROPERTY} applies only to specification \
                     renderer targets; a Typst target is its PDF"
                );
                Ok(Self::Compiled)
            },
            Renderer::Specification => {
                Ok(match declared {
                    Some(false) => Self::Omitted,
                    Some(true) | None => Self::Compiled,
                })
            },
            _ => {
                ensure!(
                    declared.is_none(),
                    "{target} ga:{PROPERTY} applies only to specification \
                     renderer targets"
                );
                Ok(Self::Omitted)
            },
        }
    }

    async fn declared(
        view: &RevisionView,
        target: &NamedNode,
    ) -> Result<Option<bool>> {
        match view.optional(target, PROPERTY).await? {
            None => Ok(None),
            Some(Term::Literal(value))
                if value.datatype().as_str() == XSD_BOOLEAN =>
            {
                match value.value() {
                    "true" | "1" => Ok(Some(true)),
                    "false" | "0" => Ok(Some(false)),
                    other => bail!("{target} invalid ga:{PROPERTY} {other}"),
                }
            },
            Some(_) => {
                bail!("{target} ga:{PROPERTY} requires an xsd:boolean literal")
            },
        }
    }
}
