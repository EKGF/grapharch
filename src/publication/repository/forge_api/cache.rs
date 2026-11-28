//! Forge responses kept per repository and commit, so a rebuild of the same
//! commit reads recorded input instead of live API data.
use {
    super::ForgeMetadata,
    crate::publication::repository::forge::ForgeRepository,
    anyhow::{Context, Result},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
};

/// A directory of recorded forge metadata.
#[derive(Clone, Debug)]
pub(crate) struct ForgeCache {
    directory: Option<PathBuf>,
}

impl ForgeCache {
    #[cfg(test)]
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self { directory: Some(directory) }
    }

    /// No recording: every request goes to the forge.
    #[cfg(test)]
    pub(crate) fn disabled() -> Self { Self { directory: None } }

    /// `$XDG_CACHE_HOME/grapharch/forge`, else `~/.cache/grapharch/forge`.
    pub(crate) fn user(environment: &dyn Fn(&str) -> Option<String>) -> Self {
        let directory = environment("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                environment("HOME")
                    .map(|home| PathBuf::from(home).join(".cache"))
            })
            .map(|base| base.join("grapharch/forge"));
        Self { directory }
    }

    pub(super) fn key(
        &self,
        base: &url::Url,
        repository: &ForgeRepository,
        commit: &str,
    ) -> String {
        let mut hash = Sha256::new();
        for part in [base.as_str(), &repository.path(), commit] {
            hash.update(part.as_bytes());
            hash.update([0]);
        }
        hex::encode(hash.finalize())
    }

    pub(super) async fn load(
        &self,
        key: &str,
    ) -> Result<Option<ForgeMetadata>> {
        let Some(directory) = &self.directory else {
            return Ok(None)
        };
        match tokio::fs::read(directory.join(format!("{key}.json"))).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            },
            Err(error) => Err(error.into()),
        }
    }

    pub(super) async fn store(
        &self,
        key: &str,
        metadata: &ForgeMetadata,
    ) -> Result<()> {
        let Some(directory) = &self.directory else {
            return Ok(())
        };
        tokio::fs::create_dir_all(directory)
            .await
            .with_context(|| {
                format!("create forge cache {}", directory.display())
            })?;
        let temporary = directory.join(format!("{key}.json.tmp"));
        tokio::fs::write(&temporary, serde_json::to_vec(metadata)?).await?;
        tokio::fs::rename(&temporary, directory.join(format!("{key}.json")))
            .await?;
        Ok(())
    }
}
