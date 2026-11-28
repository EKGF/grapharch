//! What a reference points at, and the href a renderer writes for it.
use url::Url;

/// A forge object a URL names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForgeObject {
    Issue,
    PullRequest,
    Commit,
    Release,
    Discussion,
}

/// The classified target of one reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceKind {
    /// A page of this publication, with an optional validated anchor.
    Page,
    /// A heading of the referring page.
    Anchor,
    /// A collected asset, such as an image or a downloadable file.
    Asset,
    /// The source of a rendered diagram.
    DiagramSource,
    /// A repository file that is not a page: its forge blob at the commit.
    RepoFile,
    /// An issue, pull request, commit, release or discussion on a forge.
    ForgeObject(ForgeObject),
    /// A page of a registered sibling website.
    SiblingSite,
    /// An absolute URL beneath this site's own location, made relative.
    OwnSiteAbsolute,
    /// A status badge image; fetched never, so it can never fail a build.
    Badge,
    /// Any other external location.
    External,
    /// A reference that resolves to nothing.
    Broken(String),
}

impl ReferenceKind {
    /// The kind's local name in the GraphArch vocabulary.
    pub(crate) fn term(&self) -> &'static str {
        match self {
            Self::Page => "PageLink",
            Self::Anchor => "AnchorLink",
            Self::Asset => "AssetLink",
            Self::DiagramSource => "DiagramSourceLink",
            Self::RepoFile => "RepositoryFileLink",
            Self::ForgeObject(_) => "ForgeObjectLink",
            Self::SiblingSite => "SiblingSiteLink",
            Self::OwnSiteAbsolute => "OwnSiteLink",
            Self::Badge => "BadgeLink",
            Self::External => "ExternalLink",
            Self::Broken(_) => "BrokenLink",
        }
    }
}

/// A decision: the kind and, unless broken, the href to render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Classification {
    pub(crate) kind: ReferenceKind,
    /// Relative to the referring page, or absolute for external kinds.
    pub(crate) href: Option<String>,
}

impl Classification {
    pub(crate) fn new(kind: ReferenceKind, href: impl Into<String>) -> Self {
        Self { kind, href: Some(href.into()) }
    }

    pub(crate) fn external(kind: ReferenceKind, url: &Url) -> Self {
        Self::new(kind, url.as_str())
    }

    pub(crate) fn broken(reason: impl Into<String>) -> Self {
        Self {
            kind: ReferenceKind::Broken(reason.into()),
            href: None,
        }
    }
}
