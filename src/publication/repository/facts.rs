//! Everything discovery learned about a repository, and the typed
//! diagnostics for what it could not learn.
use {
    super::{
        forge::{ForgeRepository, GitRef},
        forge_api::ForgeMetadata,
        git_facts::GitFacts,
        manifest::Manifests,
        package::{CodePackage, PackageVersion, SpdxExpression},
    },
    url::Url,
};

/// Why discovery fell back to less metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticCode {
    /// The forge API was unreachable, refused, or returned unusable data.
    ForgeUnavailable,
    /// Forge access was disabled for a reproducible build.
    ForgeOffline,
    /// A manifest could not be parsed.
    ManifestInvalid,
    /// The Git work tree could not be read.
    GitUnreadable,
}

impl DiagnosticCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ForgeUnavailable => "repository-forge-unavailable",
            Self::ForgeOffline => "repository-forge-offline",
            Self::ManifestInvalid => "repository-manifest-invalid",
            Self::GitUnreadable => "repository-git-unreadable",
        }
    }
}

/// A recorded degradation; it never fails the build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepositoryDiagnostic {
    pub(crate) code:    DiagnosticCode,
    pub(crate) message: String,
}

/// Facts of one repository: Git, forge and manifests.
#[derive(Clone, Debug, Default)]
pub(crate) struct RepositoryFacts {
    pub(crate) git:         Option<GitFacts>,
    pub(crate) forge:       Option<ForgeRepository>,
    pub(crate) metadata:    Option<ForgeMetadata>,
    pub(crate) manifests:   Manifests,
    pub(crate) diagnostics: Vec<RepositoryDiagnostic>, /* bounded: one per
                                                        * source. */
}

impl RepositoryFacts {
    pub(crate) fn primary(&self) -> Option<&CodePackage> {
        self.manifests.primary()
    }

    /// Whether a manifest names the project, which then outranks other titles.
    pub(crate) fn has_package(&self) -> bool { self.primary().is_some() }

    /// The project name: the primary package, else the forge repository.
    pub(crate) fn name(&self) -> Option<String> {
        self.primary()
            .map(|package| package.name.as_str().to_owned())
            .or_else(|| {
                self.forge.as_ref().map(|forge| forge.name().to_owned())
            })
    }

    pub(crate) fn description(&self) -> Option<String> {
        self.primary()
            .and_then(|package| package.description.clone())
            .or_else(|| {
                self.manifests.common(|package| package.description.clone())
            })
            .or_else(|| {
                self.metadata.as_ref().and_then(|m| m.description.clone())
            })
    }

    /// The package version: the primary package's, else one shared by every
    /// workspace member, else that of the latest release tag at or before
    /// the checked-out commit. The forge's latest release is not used: it
    /// may postdate the commit being documented.
    pub(crate) fn version(&self) -> Option<PackageVersion> {
        self.primary()
            .and_then(|package| package.version.clone())
            .or_else(|| {
                self.manifests.common(|package| package.version.clone())
            })
            .or_else(|| {
                let tag = self.git.as_ref()?.latest_tag.as_ref()?;
                PackageVersion::try_new(&tag.version.to_string()).ok()
            })
    }

    pub(crate) fn license(&self) -> Option<SpdxExpression> {
        self.primary()
            .and_then(|package| package.license.clone())
            .or_else(|| {
                self.manifests.common(|package| package.license.clone())
            })
            .or_else(|| self.metadata.as_ref().and_then(ForgeMetadata::license))
    }

    pub(crate) fn homepage(&self) -> Option<Url> {
        self.primary()
            .and_then(|package| package.homepage.clone())
            .or_else(|| self.metadata.as_ref().and_then(|m| m.homepage.clone()))
    }

    /// The latest release's page and tag.
    pub(crate) fn release(&self) -> Option<(GitRef, Option<Url>)> {
        if let Some(release) = self.metadata.as_ref()?.latest_release.as_ref() &&
            let Some(tag) = release.tag()
        {
            let notes = release.notes.clone().or_else(|| {
                self.forge.as_ref().map(|forge| forge.release_url(&tag))
            });
            return Some((tag, notes));
        }
        None
    }

    pub(crate) fn record(&mut self, code: DiagnosticCode, message: String) {
        tracing::warn!(code = code.as_str(), "{message}");
        self.diagnostics
            .push(RepositoryDiagnostic { code, message });
    }
}
