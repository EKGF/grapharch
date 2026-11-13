//! Collectors for the authored pages and assets beneath a project's content
//! root, shared by website builds and family builds that publish a website.
use {
    super::discovery,
    crate::publication::{
        collect::{
            Collector,
            LocalImageFile,
            MarkdownFile,
            TextAssetFile,
            git::RepositoryScan,
        },
        dataset::identity,
        view::RevisionView,
    },
    anyhow::Result,
    oxrdf::NamedNode,
    std::{path::Path, sync::Arc},
};

/// Authored site sources beneath `source`, resolved through the project's
/// `ga:contentSource` (the source root itself when undeclared).
pub(crate) struct SitePages<'a> {
    /// Root against which the content source and include roots resolve.
    pub(crate) source:          &'a Path,
    /// Pinned repository whose blobs identify the files materialized at
    /// `source`.
    pub(crate) repository:      Option<&'a RepositoryScan>,
    pub(crate) mkdocs_metadata: bool,
    /// A discovered file that is configuration rather than content.
    pub(crate) skip:            Option<&'a Path>,
}

impl SitePages<'_> {
    /// One collector per discovered page or asset, plus the blob provenance of
    /// each file read from a pinned repository.
    pub(crate) async fn collectors(
        &self,
        view: &RevisionView,
        project: &str,
    ) -> Result<Vec<Arc<dyn Collector>>> {
        let project_id = NamedNode::new(project)?;
        let content =
            discovery::content_root(self.source, view, &project_id).await?;
        let include_roots =
            discovery::include_roots(self.source, view, &project_id).await?;
        let image_bindings = crate::publication::collect::plantuml::bindings(
            view,
            project,
            self.source,
            &content,
        )
        .await?;
        let paths = discovery::discover(&content, view, &project_id).await?;
        // bounded: discovery admits at most 256 sources, each with at most one
        // provenance record.
        let mut work: Vec<Arc<dyn Collector>> = Vec::new();
        for path in paths {
            if self.skip == Some(path.as_path()) {
                continue;
            }
            let relative = crate::publication::collect::page::source_path(
                &content, &path,
            )?;
            let collector =
                identity("website-collector", &[project, &relative]);
            let blob = if let Some(repository) = self.repository {
                Some(repository.blob(path.strip_prefix(self.source)?).await?)
            } else {
                None
            };
            if let Some(blob) = &blob {
                work.push(blob.provenance_collector());
            }
            if path.extension().is_some_and(|extension| extension == "md") {
                let mut markdown = MarkdownFile::in_site(
                    project,
                    collector.as_str(),
                    content.clone(),
                    path,
                )?
                .with_default_title()
                .with_mkdocs_metadata(self.mkdocs_metadata)
                .with_image_bindings(image_bindings.clone())
                .with_include_roots(include_roots.clone())?;
                if let Some(blob) = blob {
                    markdown = markdown.with_source_identity(blob.source())?;
                }
                work.push(Arc::new(markdown));
            } else if LocalImageFile::supported(&path) {
                if let Some(blob) = blob {
                    work.push(blob.image_collector(
                        collector.as_str(),
                        identity("local-image-asset", &[project, &relative]),
                    )?);
                } else {
                    work.push(Arc::new(
                        LocalImageFile::new(project, &content, &path).await?,
                    ));
                }
            } else {
                let mut asset = TextAssetFile::new(
                    project,
                    collector.as_str(),
                    content.clone(),
                    path,
                )?;
                if let Some(blob) = blob {
                    asset = asset.with_source_identity(blob.source())?;
                }
                work.push(Arc::new(asset));
            }
        }
        Ok(work)
    }
}
