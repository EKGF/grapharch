//! Bounded transport for one blob at an already pinned repository revision.
use {
    super::*,
    crate::publication::collect::{
        MarkdownFile,
        source::{FileInput, MAX_AUTHORED_BYTES},
    },
};

#[derive(Clone)]
pub struct PinnedGitBlob {
    repository: PathBuf,
    relative:   String,
    blob:       gix::hash::ObjectId,
    revision:   Arc<Revision>,
    source:     String,
    location:   String,
}
impl RepositoryScan {
    /// Resolve a regular file within this exact fetched commit. Never
    /// re-fetches or reads a working tree, and never follows symlinks or
    /// submodules.
    pub async fn blob(&self, relative: &Path) -> Result<PinnedGitBlob> {
        PinnedGitBlob::at(self.root.clone(), self.revision.clone(), relative)
            .await
    }
}
impl PinnedGitBlob {
    async fn at(
        root: PathBuf,
        revision: Arc<Revision>,
        relative: &Path,
    ) -> Result<Self> {
        ensure!(
            !revision.working_tree,
            "Git blob transport requires a remote pinned tree"
        );
        ensure!(
            !relative.as_os_str().is_empty() &&
                relative
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "Git blob path must be source-relative without traversal"
        );
        let path = root.clone();
        let commit = revision.commit.clone();
        let relative = relative
            .to_str()
            .context("Git blob path must be UTF-8")?
            .to_owned();
        let requested = relative.clone();
        let blob = tokio::task::spawn_blocking(move || -> Result<_> {
            let repo = gix::open(path)?;
            let tree = repo
                .find_commit(gix::hash::ObjectId::from_hex(commit.as_bytes())?)?
                .tree()?;
            let entry =
                tree.lookup_entry_by_path(&requested)?.with_context(|| {
                    format!("missing required Git blob {requested}")
                })?;
            ensure!(
                matches!(
                    entry.mode().kind(),
                    gix::objs::tree::EntryKind::Blob |
                        gix::objs::tree::EntryKind::BlobExecutable
                ),
                "Git source must be a regular blob: {requested}"
            );
            Ok(entry.object_id())
        })
        .await??;
        Ok(PinnedGitBlob {
            repository: root.clone(),
            source: identity("git-file", &[&revision.origin, &relative])
                .into_string(),
            location: blob_url(&revision.origin, &revision.commit, &relative)?,
            relative,
            blob,
            revision: revision.clone(),
        })
    }

    pub fn commit(&self) -> &str { &self.revision.commit }

    pub fn source(&self) -> &str { &self.source }

    pub fn location(&self) -> &str { &self.location }

    pub fn revision(&self) -> String {
        format!(
            "git-blob-v1:{}:{}",
            self.revision.commit, self.blob
        )
    }

    pub async fn bytes(&self, limit: u64) -> Result<Vec<u8>> {
        ensure!(
            limit > 0 && limit <= MAX_RDF_BYTES,
            "invalid Git blob byte limit"
        );
        let (path, id) = (self.repository.clone(), self.blob);
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let repo = gix::open(path)?;
            ensure!(
                repo.find_header(id)?.size() <= limit,
                "Git blob exceeds configured byte limit"
            );
            let mut blob = repo.find_object(id)?.try_into_blob()?;
            ensure!(
                blob.data.len() as u64 <= limit,
                "Git blob exceeds configured byte limit"
            );
            Ok(std::mem::take(&mut blob.data))
        })
        .await?
    }

    /// Retain one explicitly configured image from this pinned repository blob.
    pub fn image_collector(
        &self,
        processor: &str,
        asset: NamedNode,
    ) -> Result<Arc<dyn Collector>> {
        NamedNode::new(processor)?;
        let mut image = GitImageFile::new(self.clone());
        image.processor = processor.into();
        image.id = asset;
        Ok(Arc::new(image))
    }

    pub fn rdf_collector(&self, processor: &str) -> Result<Arc<dyn Collector>> {
        NamedNode::new(processor)?;
        Ok(Arc::new(GitRdfFile {
            project:    self.revision.project.clone(),
            processor:  processor.into(),
            source:     self.source.clone(),
            base:       self.location.clone(),
            relative:   self.relative.clone(),
            repository: self.repository.clone(),
            blob:       self.blob,
            revision:   self.revision.as_ref().clone(),
        }))
    }

    fn relative_path(&self, destination: &str) -> Result<String> {
        ensure!(
            url::Url::parse(destination).is_err() &&
                !destination.starts_with("//"),
            "expected repository-relative image"
        );
        let base = url::Url::parse(&format!(
            "https://repository.invalid/{}",
            self.relative
        ))?;
        let resolved = base.join(destination)?;
        ensure!(
            resolved.host_str() == Some("repository.invalid") &&
                resolved.query().is_none() &&
                resolved.fragment().is_none(),
            "Git images cannot have queries or fragments"
        );
        let relative = percent_encoding::percent_decode_str(
            resolved.path().trim_start_matches('/'),
        )
        .decode_utf8()?
        .into_owned();
        ensure!(
            !relative.is_empty() &&
                Path::new(&relative)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "Git image path contains traversal"
        );
        Ok(relative)
    }

    async fn image_blob(&self, destination: &str) -> Result<Self> {
        let relative = self.relative_path(destination)?;
        Self::at(
            self.repository.clone(),
            self.revision.clone(),
            Path::new(&relative),
        )
        .await
    }

    async fn reference(
        &self,
        destination: &str,
    ) -> Result<Option<(String, String)>> {
        if destination.starts_with('#') ||
            destination.starts_with('/') ||
            url::Url::parse(destination).is_ok()
        {
            return Ok(None);
        }
        let base = url::Url::parse(&format!(
            "https://repository.invalid/{}",
            self.relative
        ))?;
        let resolved = base.join(destination)?;
        ensure!(
            resolved.host_str() == Some("repository.invalid"),
            "Git reference escaped repository"
        );
        let relative = percent_encoding::percent_decode_str(
            resolved.path().trim_start_matches('/'),
        )
        .decode_utf8()?
        .into_owned();
        ensure!(
            !relative.is_empty() &&
                Path::new(&relative)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "Git reference path contains traversal"
        );
        let (repository, commit, path) = (
            self.repository.clone(),
            self.revision.commit.clone(),
            relative.clone(),
        );
        let is_tree = tokio::task::spawn_blocking(move || -> Result<bool> {
            let repo = gix::open(repository)?;
            let tree = repo
                .find_commit(gix::hash::ObjectId::from_hex(commit.as_bytes())?)?
                .tree()?;
            let entry =
                tree.lookup_entry_by_path(&path)?.with_context(|| {
                    format!("missing Git reference target {path} at {commit}")
                })?;
            match entry.mode().kind() {
                gix::objs::tree::EntryKind::Tree => Ok(true),
                gix::objs::tree::EntryKind::Blob |
                gix::objs::tree::EntryKind::BlobExecutable => Ok(false),
                _ => {
                    anyhow::bail!(
                        "Git references to links and submodules are \
                         unsupported: {path}"
                    )
                },
            }
        })
        .await??;
        let origin = self
            .revision
            .origin
            .trim_end_matches(".git")
            .trim_end_matches('/');
        ensure!(
            origin.starts_with("https://github.com/"),
            "Git reference publication requires a GitHub HTTPS origin"
        );
        let mut target = url::Url::parse(&format!(
            "{origin}/{}/{}/",
            if is_tree { "tree" } else { "blob" },
            self.revision.commit
        ))?;
        {
            let mut segments = target.path_segments_mut().map_err(|_| {
                anyhow::anyhow!("Git origin has no hierarchical path")
            })?;
            segments.pop_if_empty();
            for part in relative.split('/') {
                segments.push(part);
            }
        }
        target.set_query(resolved.query());
        target.set_fragment(resolved.fragment());
        Ok(Some((relative, target.into())))
    }

    async fn provenance(&self, stream: &mut ContributionStream) -> Result<()> {
        for (predicate, object) in [
            (
                "http://www.w3.org/ns/prov#wasDerivedFrom",
                self.revision.entity(),
            ),
            (
                "http://www.w3.org/ns/prov#atLocation",
                NamedNode::new(&self.location)?,
            ),
        ] {
            stream
                .write(Quad::new(
                    NamedNode::new(&self.source)?,
                    NamedNode::new(predicate)?,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(())
    }
}
/// One Markdown document; transport is independent of Markdown normalization.
pub struct GitMarkdownFile {
    blob:      PinnedGitBlob,
    processor: String,
    fragment:  NamedNode,
    website:   bool,
    hints:     Option<(String, String)>,
}
impl GitMarkdownFile {
    pub fn new(
        blob: PinnedGitBlob,
        processor: &str,
        fragment: NamedNode,
        website: bool,
    ) -> Result<Self> {
        NamedNode::new(processor)?;
        Ok(Self {
            blob,
            processor: processor.into(),
            fragment,
            website,
            hints: None,
        })
    }

    pub fn fragment_id(&self) -> NamedNode { self.fragment.clone() }

    pub async fn image_collectors(&self) -> Result<Vec<Arc<dyn Collector>>> {
        let text =
            String::from_utf8(self.blob.bytes(MAX_AUTHORED_BYTES).await?)?;
        let mut urls = std::collections::BTreeSet::new();
        for event in pulldown_cmark::Parser::new_ext(
            &text,
            crate::publication::markdown::options(),
        ) {
            if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image {
                dest_url,
                ..
            }) = event
            {
                urls.insert(dest_url.to_string());
            }
        }
        ensure!(
            urls.len() <= 64,
            "Markdown exceeds 64-image budget"
        );
        let mut collectors = Vec::new();
        for url in urls {
            collectors.push(self.image(&url).await?.2);
        }
        Ok(collectors)
    }

    async fn image(
        &self,
        destination: &str,
    ) -> Result<(NamedNode, String, Arc<dyn Collector>)> {
        if url::Url::parse(destination).is_ok() {
            let image = crate::publication::collect::HttpImage::new(
                &self.blob.revision.project,
                destination,
            )?;
            Ok((image.id(), destination.into(), Arc::new(image)))
        } else {
            let image =
                GitImageFile::new(self.blob.image_blob(destination).await?);
            Ok((
                image.id.clone(),
                image.id.as_str().into(),
                Arc::new(image),
            ))
        }
    }

    pub fn with_page_hints(
        mut self,
        title: String,
        route: String,
    ) -> Result<Self> {
        ensure!(
            self.website && !title.trim().is_empty() && title.len() <= 1024,
            "invalid Git Markdown page hints"
        );
        super::super::page::route(&self.blob.relative, Some(&route))?;
        self.hints = Some((title, route));
        Ok(self)
    }
}
#[async_trait]
impl Collector for GitMarkdownFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.blob.revision.project,
            processor: &self.processor,
            source:    &self.blob.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let text =
            String::from_utf8(self.blob.bytes(MAX_AUTHORED_BYTES).await?)
                .context("Git Markdown must be UTF-8")?;
        let hints = self.hints.as_ref();
        let revision = identity("git-markdown-revision-v2", &[
            &self.blob.revision(),
            self.fragment.as_str(),
            if self.website { "website" } else { "fragment" },
            hints.map(|v| v.0.as_str()).unwrap_or(""),
            hints.map(|v| v.1.as_str()).unwrap_or(""),
        ])
        .into_string();
        let input = FileInput::supplied(
            &self.blob.revision.project,
            &self.processor,
            PathBuf::from(&self.blob.relative),
            self.blob.source.clone(),
            self.fragment.clone(),
            text.clone(),
            revision,
        )?;
        let revision =
            MarkdownFile::supplied(input, self.website, self.hints.clone())
                .produce(stream)
                .await?;
        let image_destinations = pulldown_cmark::Parser::new_ext(
            &text,
            crate::publication::markdown::options(),
        )
        .filter_map(|event| {
            match event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image {
                    dest_url,
                    ..
                }) => Some(dest_url.to_string()),
                _ => None,
            }
        })
        .collect::<std::collections::BTreeSet<_>>();
        ensure!(
            image_destinations.len() <= 64,
            "Markdown exceeds image budget"
        );
        let mut count = 0;
        let mut seen = std::collections::BTreeSet::new();
        for event in pulldown_cmark::Parser::new_ext(
            &text,
            pulldown_cmark::Options::all(),
        ) {
            let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link {
                dest_url,
                ..
            }) = event
            else {
                continue
            };
            if image_destinations.contains(dest_url.as_ref()) {
                continue;
            }
            if !seen.insert(dest_url.to_string()) {
                continue;
            }
            count += 1;
            ensure!(
                count <= 1024,
                "Git Markdown exceeds reference budget"
            );
            if let Some((path, target)) = self.blob.reference(&dest_url).await?
            {
                let reference = identity("git-markdown-reference", &[
                    self.fragment.as_str(),
                    &dest_url,
                ]);
                for (subject, predicate, object) in [
                    (
                        self.fragment.clone(),
                        crate::publication::term("reference"),
                        Term::NamedNode(reference.clone()),
                    ),
                    (
                        reference.clone(),
                        NamedNode::new_unchecked(crate::publication::RDF_TYPE),
                        Term::NamedNode(crate::publication::term(
                            "ExternalReference",
                        )),
                    ),
                    (
                        reference.clone(),
                        NamedNode::new_unchecked(
                            "http://purl.org/dc/terms/identifier",
                        ),
                        Literal::new_simple_literal(dest_url.to_string())
                            .into(),
                    ),
                    (
                        reference.clone(),
                        crate::publication::term("sourcePath"),
                        Literal::new_simple_literal(path).into(),
                    ),
                    (
                        reference,
                        crate::publication::term("url"),
                        Literal::new_simple_literal(target).into(),
                    ),
                ] {
                    stream
                        .write(Quad::new(
                            subject,
                            predicate,
                            object,
                            GraphName::DefaultGraph,
                        ))
                        .await?;
                }
            }
        }
        for event in pulldown_cmark::Parser::new_ext(
            &text,
            crate::publication::markdown::options(),
        ) {
            if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image {
                dest_url,
                ..
            }) = event
            {
                let (asset, url, _) = self.image(&dest_url).await?;
                let reference = identity("git-markdown-reference", &[
                    self.fragment.as_str(),
                    &dest_url,
                ]);
                for (subject, predicate, object) in [
                    (
                        self.fragment.clone(),
                        crate::publication::term("reference"),
                        Term::NamedNode(reference.clone()),
                    ),
                    (
                        self.fragment.clone(),
                        crate::publication::term("asset"),
                        Term::NamedNode(asset.clone()),
                    ),
                    (
                        reference.clone(),
                        NamedNode::new_unchecked(crate::publication::RDF_TYPE),
                        Term::NamedNode(crate::publication::term(
                            "ExternalReference",
                        )),
                    ),
                    (
                        reference.clone(),
                        NamedNode::new_unchecked(
                            "http://purl.org/dc/terms/identifier",
                        ),
                        Literal::new_simple_literal(dest_url.to_string())
                            .into(),
                    ),
                    (
                        reference.clone(),
                        crate::publication::term("url"),
                        Literal::new_simple_literal(url).into(),
                    ),
                    (
                        reference,
                        crate::publication::term("asset"),
                        Term::NamedNode(asset.clone()),
                    ),
                ] {
                    stream
                        .write(Quad::new(
                            subject,
                            predicate,
                            object,
                            GraphName::DefaultGraph,
                        ))
                        .await?;
                }
            }
        }
        self.blob.provenance(stream).await?;
        Ok(revision)
    }
}

struct GitImageFile {
    blob:      PinnedGitBlob,
    processor: String,
    id:        NamedNode,
}
impl GitImageFile {
    fn new(blob: PinnedGitBlob) -> Self {
        Self {
            processor: identity("git-image-collector", &[
                &blob.revision.project,
                &blob.source,
            ])
            .into_string(),
            id: identity("image-asset", &[
                &blob.revision.project,
                &blob.source,
            ]),
            blob,
        }
    }
}
#[async_trait]
impl Collector for GitImageFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.blob.revision.project,
            processor: &self.processor,
            source:    &self.blob.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let bytes = self
            .blob
            .bytes(crate::publication::image_assets::MAX_IMAGE_BYTES as u64)
            .await?;
        let extension = Path::new(&self.blob.relative)
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = match extension.as_str() {
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            _ => anyhow::bail!("unsupported Git image extension: {extension}"),
        };
        let revision = super::super::image::ImageAsset {
            project:   &self.blob.revision.project,
            processor: &self.processor,
            source:    &self.blob.source,
            id:        &self.id,
        }
        .write(&bytes, mime, stream)
        .await?;
        self.blob.provenance(stream).await?;
        Ok(format!(
            "git-image-v1:{}:{revision}",
            self.blob.revision()
        ))
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[tokio::test]
    async fn repository_links_are_verified_at_the_pinned_commit() -> Result<()>
    {
        let source = tempfile::tempdir()?;
        let cache = tempfile::tempdir()?;
        let run = |args: &[&str]| -> Result<()> {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(source.path())
                .args(args)
                .output()?;
            ensure!(
                output.status.success(),
                "fixture git failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(())
        };
        run(&["init", "-b", "develop"])?;
        run(&["config", "user.name", "Fixture"])?;
        run(&["config", "user.email", "fixture@example.org"])?;
        std::fs::write(
            source.path().join("README.md"),
            "# Readme\n[guide](guide.md#example)\n",
        )?;
        std::fs::write(source.path().join("guide.md"), "# Guide\n")?;
        for path in [
            "space name.md",
            "literal+plus.md",
            "literal%name.md",
            "café.md",
        ] {
            std::fs::write(source.path().join(path), "# Encoded path\n")?;
        }

        std::fs::write(
            source.path().join("ontology.ttl"),
            "<urn:test:a> <urn:test:p> <urn:test:b> .",
        )?;
        run(&["add", "."])?;
        run(&["commit", "-m", "fixture"])?;
        let scan = super::super::remote_repository(
            source.path().to_str().unwrap(),
            "develop",
            cache.path(),
            "urn:test:project",
        )
        .await?;
        let mut blob = scan.blob(Path::new("README.md")).await?;
        let mut revision = blob.revision.as_ref().clone();
        revision.origin = "https://github.com/example/ontology.git".into();
        blob.revision = Arc::new(revision);
        let (path, url) = blob.reference("guide.md#example").await?.unwrap();
        assert_eq!(path, "guide.md");
        assert_eq!(
            url,
            format!(
                "https://github.com/example/ontology/blob/{}/guide.md#example",
                scan.commit
            )
        );
        for (source, path) in [
            ("space%20name.md", "space name.md"),
            ("literal+plus.md", "literal+plus.md"),
            ("literal%2Bplus.md", "literal+plus.md"),
            ("literal%25name.md", "literal%name.md"),
            ("caf%C3%A9.md", "café.md"),
        ] {
            let (actual, url) = blob.reference(source).await?.unwrap();
            assert_eq!(actual, path);
            assert!(url.contains(&scan.commit));
        }
        assert!(blob.reference("missing.md").await.is_err());
        assert!(blob.reference("#local").await?.is_none());
        assert!(blob.reference("https://example.org/guide").await?.is_none());
        Ok(())
    }
}
