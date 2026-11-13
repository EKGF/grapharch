//! Where a target's definitions retain the documentation graph inside its
//! publication, so pages can link to it with a portable relative reference.
use {
    super::{dataset::Dataset, render::Publication, view::RevisionView},
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

/// A validated `ga:documentationGraphPath`: a relative N-Quads file path
/// beneath the target's destination.
pub(crate) struct GraphPlacement {
    relative: String,
}

impl GraphPlacement {
    /// Read the target's declared placement, if any.
    pub(crate) async fn of(
        view: &RevisionView,
        target: &NamedNode,
    ) -> Result<Option<Self>> {
        match view.optional(target, "documentationGraphPath").await? {
            None => Ok(None),
            Some(Term::Literal(path)) => {
                Self::try_new(path.value())
                    .with_context(|| {
                        format!("{target} ga:documentationGraphPath")
                    })
                    .map(Some)
            },
            Some(_) => {
                bail!("{target} ga:documentationGraphPath must be a literal")
            },
        }
    }

    /// Retain the exported graph where the publication's target places it.
    pub(crate) async fn retain_declared(
        dataset: &Dataset,
        publication: &Publication,
        archive: &Path,
    ) -> Result<Option<String>> {
        let view = RevisionView::new(
            dataset.clone(),
            NamedNode::new(&publication.manifest.input_revision)?,
        );
        let target = NamedNode::new(&publication.manifest.target)?;
        match Self::of(&view, &target).await? {
            Some(placement) => {
                Ok(Some(
                    placement.retain(publication, archive).await?,
                ))
            },
            None => Ok(None),
        }
    }

    fn try_new(value: &str) -> Result<Self> {
        let path = Path::new(value);
        ensure!(
            !value.is_empty() &&
                value.len() <= 1024 &&
                !value.contains('\\') &&
                path.components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "documentation graph path {value:?} must be relative, separated \
             by / and free of traversal"
        );
        ensure!(
            path.extension().is_some_and(|extension| extension == "nq"),
            "documentation graph path {value:?} must name an N-Quads (.nq) \
             file"
        );
        Ok(Self { relative: value.to_owned() })
    }

    /// Copy the exported documentation graph into the publication and return
    /// its path relative to the publication directory. A publication file of
    /// the same name is a conflict, not something to overwrite.
    pub(crate) async fn retain(
        &self,
        publication: &Publication,
        archive: &Path,
    ) -> Result<String> {
        ensure!(
            !publication.manifest.files.contains(&self.relative),
            "documentation graph path {} conflicts with a published file",
            self.relative
        );
        let destination: PathBuf = publication.directory.join(&self.relative);
        ensure!(
            tokio::fs::symlink_metadata(&destination).await.is_err(),
            "documentation graph path {} already exists",
            self.relative
        );
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(archive, &destination)
            .await
            .with_context(|| {
                format!(
                    "retain documentation graph at {}",
                    destination.display()
                )
            })?;
        Ok(self.relative.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::GraphPlacement;

    #[test]
    fn placements_are_portable_relative_n_quads_paths() {
        for value in ["documentation.nq", "graph/documentation.nq"] {
            assert!(GraphPlacement::try_new(value).is_ok(), "{value}");
        }
        for value in [
            "",
            "/documentation.nq",
            "../documentation.nq",
            "./documentation.nq",
            "graph\\documentation.nq",
            "documentation.ttl",
        ] {
            assert!(GraphPlacement::try_new(value).is_err(), "{value}");
        }
    }
}
