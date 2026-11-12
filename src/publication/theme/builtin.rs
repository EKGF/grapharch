//! The engine-owned theme catalogue in `docs/other/ontology/themes.ttl`.
use {crate::publication::term, oxrdf::NamedNode};

/// A catalogue theme. Its identity also selects the bundled HTML and website
/// skins; consumer themes inherit the skin of their nearest catalogue ancestor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    Reader,
    Specification,
    Overview,
    Ekgf,
    EkgfProject,
}

impl Builtin {
    // bounded: the fixed catalogue.
    const ALL: [Self; 5] = [
        Self::Reader,
        Self::Specification,
        Self::Overview,
        Self::Ekgf,
        Self::EkgfProject,
    ];

    fn local(self) -> &'static str {
        match self {
            Self::Reader => "ReaderTheme",
            Self::Specification => "SpecificationTheme",
            Self::Overview => "OverviewTheme",
            Self::Ekgf => "EkgfTheme",
            Self::EkgfProject => "EkgfProjectTheme",
        }
    }

    pub(crate) fn iri(self) -> NamedNode { term(self.local()) }

    pub(crate) fn from_iri(iri: &NamedNode) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| &theme.iri() == iri)
    }
}
