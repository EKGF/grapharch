//! The declared conformance language of a publication target.
use {
    crate::publication::{term, view::RevisionView},
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};

/// A conformance language a target declares with `ga:conformanceLanguage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::publication::render) enum ConformanceLanguage {
    /// `ga:Bcp14`: the requirement levels of RFC 2119 as clarified by RFC
    /// 8174.
    Bcp14,
}

impl ConformanceLanguage {
    /// bounded: the fixed catalogue.
    const ALL: [Self; 1] = [Self::Bcp14];

    fn iri(self) -> NamedNode {
        match self {
            Self::Bcp14 => term("Bcp14"),
        }
    }

    fn from_iri(iri: &NamedNode) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|language| &language.iri() == iri)
            .ok_or_else(|| {
                anyhow::anyhow!("unsupported conformance language {iri}")
            })
    }
}

/// Whether a target's prose uses declared normative keywords. Without a
/// declaration, rendering leaves keywords and native markup unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::publication::render) struct Conformance {
    language: Option<ConformanceLanguage>,
}

impl Conformance {
    #[cfg(test)]
    pub(in crate::publication::render) const BCP14: Self =
        Self { language: Some(ConformanceLanguage::Bcp14) };

    /// Read the target's optional, single declaration.
    pub(in crate::publication::render) async fn read(
        view: &RevisionView,
        target: &NamedNode,
    ) -> Result<Self> {
        let language =
            match view.optional(target, "conformanceLanguage").await? {
                None => None,
                Some(Term::NamedNode(iri)) => {
                    Some(ConformanceLanguage::from_iri(&iri)?)
                },
                Some(_) => {
                    bail!("{target} ga:conformanceLanguage must be an IRI")
                },
            };
        Ok(Self { language })
    }

    /// Whether BCP 14 keywords are presented as normative.
    pub(in crate::publication::render) fn bcp14(self) -> bool {
        self.language == Some(ConformanceLanguage::Bcp14)
    }
}
