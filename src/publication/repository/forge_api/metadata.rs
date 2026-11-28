//! What a forge API says about a repository, independent of the forge.
use {
    crate::publication::repository::{forge::GitRef, package::SpdxExpression},
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    url::Url,
};

/// Who may see the repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Visibility {
    Public,
    Internal,
    Private,
}

impl Visibility {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "public" => Some(Self::Public),
            "internal" => Some(Self::Internal),
            "private" => Some(Self::Private),
            _ => None,
        }
    }

    pub(crate) fn term(self) -> &'static str {
        match self {
            Self::Public => "PublicRepository",
            Self::Internal => "InternalRepository",
            Self::Private => "PrivateRepository",
        }
    }
}

/// The latest published release of a repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ForgeRelease {
    pub(crate) name:  Option<String>,
    pub(crate) tag:   String,
    pub(crate) date:  Option<DateTime<Utc>>,
    pub(crate) notes: Option<Url>,
}

impl ForgeRelease {
    pub(crate) fn tag(&self) -> Option<GitRef> {
        GitRef::try_new(&self.tag).ok()
    }
}

/// Forge-independent repository metadata, cached as recorded input.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ForgeMetadata {
    pub(crate) description:    Option<String>,
    pub(crate) homepage:       Option<Url>,
    pub(crate) topics:         Vec<String>,
    pub(crate) license:        Option<String>,
    pub(crate) visibility:     Option<Visibility>,
    pub(crate) archived:       bool,
    pub(crate) stars:          Option<u64>,
    pub(crate) discussions:    bool,
    pub(crate) latest_release: Option<ForgeRelease>,
}

impl ForgeMetadata {
    pub(crate) fn license(&self) -> Option<SpdxExpression> {
        self.license
            .as_deref()
            .filter(|id| *id != "NOASSERTION" && *id != "other")
            .and_then(|id| SpdxExpression::try_new(id).ok())
    }
}
