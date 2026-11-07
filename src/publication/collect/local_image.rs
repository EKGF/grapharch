//! Bounded source-root images, retained through the shared binary-asset format.
use {
    super::{Collector, ImageAsset, SourceIdentity},
    crate::publication::{
        dataset::identity,
        image_assets::MAX_IMAGE_BYTES,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    oxrdf::NamedNode,
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

/// One SVG, PNG, or JPEG image beneath an explicitly selected source root.
pub struct LocalImageFile {
    project:   String,
    processor: String,
    source:    String,
    root:      PathBuf,
    path:      PathBuf,
    id:        NamedNode,
}

impl LocalImageFile {
    /// Validate a source-owned image and identify it by its root-relative path.
    pub async fn new(project: &str, root: &Path, path: &Path) -> Result<Self> {
        NamedNode::new(project)?;
        let root = tokio::fs::canonicalize(root).await?;
        let path = tokio::fs::canonicalize(path).await.with_context(|| {
            format!("local Markdown image {}", path.display())
        })?;
        ensure!(
            path.starts_with(&root),
            "local Markdown image escapes documentation root"
        );
        ensure!(
            tokio::fs::metadata(&path).await?.is_file(),
            "local Markdown image must be a regular file"
        );
        Self::mime(&path)?;
        let source = super::page::source_path(&root, &path)?;
        Ok(Self {
            project: project.into(),
            processor: identity("local-image-collector", &[project, &source])
                .into_string(),
            id: identity("local-image-asset", &[project, &source]),
            source,
            root,
            path,
        })
    }

    pub(super) async fn reference(
        project: &str,
        root: Option<&Path>,
        source: &Path,
        destination: &str,
    ) -> Result<Self> {
        ensure!(
            !destination.starts_with("//") && !destination.contains('\\'),
            "local Markdown images must use source-relative or \
             site-root-relative paths"
        );
        ensure!(
            !destination.starts_with('/') || root.is_some(),
            "site-root-relative images require an explicit documentation root"
        );
        let source = tokio::fs::canonicalize(source).await?;
        let root = root.unwrap_or_else(|| {
            source.parent().expect("canonical file has a parent")
        });
        let base = url::Url::from_file_path(&source)
            .map_err(|()| anyhow::anyhow!("invalid Markdown source path"))?;
        let resolved = if let Some(relative) = destination.strip_prefix('/') {
            let root = tokio::fs::canonicalize(root).await?;
            url::Url::from_directory_path(root)
                .map_err(|()| anyhow::anyhow!("invalid documentation root"))?
                .join(relative)?
        } else {
            base.join(destination)?
        };
        ensure!(
            resolved.scheme() == "file" &&
                resolved.query().is_none() &&
                resolved.fragment().is_none(),
            "local Markdown images cannot have queries or fragments"
        );
        let path = resolved.to_file_path().map_err(|()| {
            anyhow::anyhow!("invalid local Markdown image path")
        })?;
        Self::new(project, root, &path).await
    }

    /// Stable project-scoped retained-image identity.
    pub fn id(&self) -> NamedNode { self.id.clone() }

    fn mime(path: &Path) -> Result<&'static str> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("svg") => Ok("image/svg+xml"),
            Some("png") => Ok("image/png"),
            Some("jpg" | "jpeg") => Ok("image/jpeg"),
            _ => {
                anyhow::bail!("local Markdown image must be SVG, PNG, or JPEG")
            },
        }
    }

    pub(crate) fn supported(path: &Path) -> bool { Self::mime(path).is_ok() }
}

#[async_trait]
impl Collector for LocalImageFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let result = async {
            let path = tokio::fs::canonicalize(&self.path).await?;
            ensure!(
                path.starts_with(&self.root),
                "local Markdown image escapes documentation root"
            );
            let file = tokio::fs::File::open(&path).await?;
            ensure!(
                file.metadata().await?.len() <= MAX_IMAGE_BYTES as u64,
                "image exceeds 8 MiB budget"
            );
            let mut bytes = Vec::new(); // bounded: image reader caps input at 8 MiB plus one overflow byte.
            file.take(MAX_IMAGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .await?;
            ImageAsset {
                project:   &self.project,
                processor: &self.processor,
                source:    &self.source,
                id:        &self.id,
            }
            .write(&bytes, Self::mime(&path)?, stream)
            .await
        }
        .await;
        result.with_context(|| format!("local image {}", self.source))
    }
}
