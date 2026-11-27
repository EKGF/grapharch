//! Whether documentation draws OWL class diagrams, as declared in RDF.
use {
    crate::publication::{
        document::plantuml::LocalRenderer,
        term,
        view::RevisionView,
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
    std::{ffi::OsStr, sync::Arc},
};

/// The `ga:OwlClassDiagramRenderer` state of one documentation build.
#[derive(Clone)]
pub(crate) enum ClassDiagrams {
    /// Draw each documented OWL class with the local PlantUML launcher.
    Drawn(Arc<LocalRenderer>),
    /// The profile disables the fragment renderer; no diagram is generated
    /// and neither PlantUML nor Java is required.
    Disabled,
}

impl ClassDiagrams {
    /// The default: draw with the `plantuml` launcher on `PATH`.
    pub(crate) fn local() -> Self {
        Self::Drawn(Arc::new(LocalRenderer::new(OsStr::new(
            "plantuml",
        ))))
    }

    /// Read `ga:disableRenderer` of a publication profile. Only a fragment
    /// renderer identity, `ga:OwlClassDiagramRenderer`, is accepted.
    pub(crate) async fn for_profile(
        view: &RevisionView,
        profile: &NamedNode,
    ) -> Result<Self> {
        let renderer = term("OwlClassDiagramRenderer");
        let mut rows = view.objects(profile, "disableRenderer").await?;
        let mut disabled = false;
        while let Some(row) = rows.next().await? {
            match &row["value"] {
                Term::NamedNode(value) if *value == renderer => disabled = true,
                other => {
                    bail!(
                        "ga:disableRenderer of profile {profile} must \
                         identify the fragment renderer {renderer}, not \
                         {other}"
                    )
                },
            }
        }
        Ok(if disabled {
            Self::Disabled
        } else {
            Self::local()
        })
    }

    /// The renderer to draw with, absent when diagrams are disabled.
    pub(crate) fn renderer(&self) -> Option<&LocalRenderer> {
        match self {
            Self::Drawn(renderer) => Some(renderer),
            Self::Disabled => None,
        }
    }
}
