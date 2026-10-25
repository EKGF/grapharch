//! One UTF-8 downloadable asset, retained in the graph for offline rendering.
use {
    super::{Collector, source::FileInput},
    crate::publication::{term, writer::ContributionStream},
    anyhow::Result,
    async_trait::async_trait,
    oxrdf::{Literal, NamedNode},
    std::path::PathBuf,
};

/// Collector for one text download, such as a vocabulary or license file.
pub struct TextAssetFile {
    input: FileInput,
    root:  PathBuf,
}
impl TextAssetFile {
    /// Register a text download under its source-relative artifact path.
    pub fn new(
        project: &str,
        collector: &str,
        root: PathBuf,
        path: PathBuf,
    ) -> Result<Self> {
        super::page::source_path(&root, &path)?;
        Ok(Self {
            input: FileInput::new(project, collector, path)?,
            root,
        })
    }

    /// Stable resource identifying this asset and its fragment.
    pub fn fragment_id(&self) -> NamedNode { self.input.fragment_id() }
}
#[async_trait]
impl Collector for TextAssetFile {
    fn identity(&self) -> Result<super::SourceIdentity<'_>> {
        self.input.identity()
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let (body, revision) = self.input.authored().await?;
        let path = super::page::source_path(&self.root, &self.input.path)?;
        let revision = format!("text-asset-v1:{path}:{revision}");
        let fragment = self.fragment_id();
        let mut quads = self.input.description(
            &body,
            "text/plain",
            Some(fragment.clone()),
            None,
            &revision,
        )?;
        quads.extend([
            self.input.quad(&fragment, "type", term("TextAsset")),
            self.input.quad(
                &fragment,
                "sourcePath",
                Literal::new_simple_literal(&path),
            ),
            self.input.quad(
                &fragment,
                "route",
                Literal::new_simple_literal(&path),
            ),
        ]);
        stream.push(quads).await?;
        Ok(revision)
    }
}
