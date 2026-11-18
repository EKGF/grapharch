//! The private staging directory one publication is written into before it
//! is promoted to its destination.
use {
    super::file_release::FileRelease,
    anyhow::Result,
    std::path::{Path, PathBuf},
    uuid::Uuid,
};

pub(super) struct Staging {
    pub path: PathBuf,
}

impl Staging {
    pub async fn new(destination: &Path) -> Result<Self> {
        let path = destination
            .with_file_name(format!(".grapharch-stage-{}", Uuid::new_v4()));
        tokio::fs::create_dir(&path).await?;
        Ok(Self { path })
    }

    /// Remove whatever remains of the stage after `outcome`, then return
    /// `outcome`. A failed publication keeps its own error as the cause; a
    /// cleanup failure is attached to it instead of replacing it.
    pub async fn finish<T>(&self, outcome: Result<T>) -> Result<T> {
        match (outcome, self.cleanup().await) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(cleanup)) => Err(cleanup),
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(error.context(format!(
                    "publication failed, and then its stage could not be \
                     removed ({cleanup:#})"
                )))
            },
        }
    }

    async fn cleanup(&self) -> Result<()> {
        match FileRelease::platform().remove_dir_all(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(())
            },
            Err(error) => {
                Err(anyhow::Error::new(error).context(format!(
                    "cannot remove publication stage {}",
                    self.path.display()
                )))
            },
        }
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let path = self.path.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = tokio::fs::remove_dir_all(&path).await &&
                    error.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::error!(
                        "cannot remove abandoned staging directory {}: {error}",
                        path.display()
                    );
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_failed_cleanup_keeps_the_publication_error_as_the_cause()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        // A file in place of the stage directory cannot be removed as one.
        let stage = Staging { path: root.path().join("stage") };
        std::fs::write(&stage.path, b"not a directory")?;
        let error = stage
            .finish::<()>(Err(anyhow::anyhow!("book worker failed")))
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("publication succeeded"))?;
        assert_eq!(
            error.root_cause().to_string(),
            "book worker failed"
        );
        assert!(
            format!("{error:#}").contains("cannot remove publication stage"),
            "{error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_promoted_stage_is_already_gone() -> Result<()> {
        let root = tempfile::tempdir()?;
        let stage = Staging::new(&root.path().join("destination")).await?;
        std::fs::remove_dir(&stage.path)?;
        stage.finish(Ok(())).await
    }
}
