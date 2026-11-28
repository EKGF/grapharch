//! Discover a repository's metadata without ever failing the build on
//! missing evidence.
use {
    super::{
        facts::{DiagnosticCode, RepositoryFacts},
        forge::ForgeRepository,
        forge_api::{ForgeAccess, ForgeCache},
        git_facts::GitFacts,
        manifest::Manifests,
    },
    std::path::Path,
};

/// Looks up an environment variable.
type Environment = dyn Fn(&str) -> Option<String> + Send + Sync;

/// Reads Git, manifests and, when allowed, the forge API.
pub(crate) struct RepositoryDiscoverer {
    access:      ForgeAccess,
    cache:       ForgeCache,
    environment: Box<Environment>,
}

impl RepositoryDiscoverer {
    /// Use the process environment for API locations, tokens and the cache.
    pub(crate) fn new(access: ForgeAccess) -> Self {
        let environment = |name: &str| std::env::var(name).ok();
        Self {
            access,
            cache: ForgeCache::user(&environment),
            environment: Box::new(environment),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_environment(
        access: ForgeAccess,
        cache: ForgeCache,
        environment: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        Self { access, cache, environment: Box::new(environment) }
    }

    /// Facts of the repository at or enclosing `source`. Manifests are read
    /// at the work-tree root, else at `source` itself.
    pub(crate) async fn discover(&self, source: &Path) -> RepositoryFacts {
        let mut facts = RepositoryFacts::default();
        match GitFacts::discover(source).await {
            Ok(git) => facts.git = git,
            Err(error) => {
                facts.record(
                    DiagnosticCode::GitUnreadable,
                    format!("Git metadata unavailable: {error:#}"),
                )
            },
        }
        let root = facts.git.as_ref().map_or(source, |git| git.root.as_path());
        match Manifests::read(root).await {
            Ok(manifests) => facts.manifests = manifests,
            Err(error) => {
                facts.record(
                    DiagnosticCode::ManifestInvalid,
                    format!("package manifest ignored: {error:#}"),
                )
            },
        }
        facts.forge = facts
            .git
            .as_ref()
            .and_then(|git| git.origin.clone())
            .or_else(|| {
                facts.manifests.packages().iter().find_map(|package| {
                    ForgeRepository::from_remote(
                        package.repository.as_ref()?.as_str(),
                    )
                    .ok()
                    .flatten()
                })
            });
        self.forge(&mut facts).await;
        facts
    }

    async fn forge(&self, facts: &mut RepositoryFacts) {
        let Some(forge) = facts.forge.clone() else {
            return
        };
        if self.access == ForgeAccess::Offline {
            facts.record(
                DiagnosticCode::ForgeOffline,
                format!(
                    "forge metadata of {} not requested (offline)",
                    forge.path()
                ),
            );
            return;
        }
        // Without a commit the response cannot be pinned, so key it by the
        // package version that identifies the copy instead.
        let pin = facts
            .git
            .as_ref()
            .map(|git| git.head.as_str().to_owned())
            .or_else(|| {
                facts.version().map(|v| format!("version:{}", v.as_str()))
            })
            .unwrap_or_else(|| "unpinned".to_owned());
        match super::forge_api::client::fetch(
            &forge,
            &pin,
            &self.cache,
            &*self.environment,
        )
        .await
        {
            Ok(metadata) => facts.metadata = Some(metadata),
            Err(error) => {
                facts.record(
                    DiagnosticCode::ForgeUnavailable,
                    format!(
                        "{} metadata of {} unavailable; using Git and \
                         manifests only: {error:#}",
                        forge.kind().label(),
                        forge.path()
                    ),
                )
            },
        }
    }
}
