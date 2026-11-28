//! The `hide` list of Material for MkDocs frontmatter: generated page regions
//! one page omits. Regions GraphArch has no counterpart for are accepted and
//! have no effect, as Material ignores regions a page does not render.
use {crate::publication::term, oxrdf::NamedNode, serde::Deserialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(super) enum HiddenRegion {
    /// The navigation sidebar.
    Navigation,
    /// The outline of the page's headings.
    Toc,
    /// Material's header title; GraphArch shows the site title.
    Title,
    /// Material's previous/next footer.
    Footer,
    /// Material's breadcrumbs.
    Path,
    /// Material's tag list.
    Tags,
    /// Material's feedback widget.
    Feedback,
}

impl HiddenRegion {
    /// The `ga:PageRegion` this hides, when GraphArch renders one.
    pub(super) fn term(self) -> Option<NamedNode> {
        match self {
            Self::Navigation => Some(term("NavigationRegion")),
            Self::Toc => Some(term("OutlineRegion")),
            Self::Title |
            Self::Footer |
            Self::Path |
            Self::Tags |
            Self::Feedback => None,
        }
    }
}
