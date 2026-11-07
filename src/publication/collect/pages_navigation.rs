//! Collect source-owned directory navigation hints without a MkDocs runtime.
mod discovery;
mod hints;
use {
    super::{Collector, SourceIdentity},
    crate::publication::{dataset::identity, term, writer::ContributionStream},
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
};

pub(super) struct PagesNavigation {
    pub project:    String,
    pub processor:  NamedNode,
    pub profile:    NamedNode,
    pub root:       PathBuf,
    pub exclusions: Vec<PathBuf>,
}
#[async_trait]
impl Collector for PagesNavigation {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: self.processor.as_str(),
            source:    self.root.to_str().ok_or_else(|| {
                anyhow::anyhow!("navigation source path must be UTF-8")
            })?,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let sidebar = identity("directory-navigation", &[
            self.profile.as_str(),
            "sidebar",
        ]);
        let primary = identity("directory-navigation", &[
            self.profile.as_str(),
            "primary",
        ]);
        for (tree, region, title) in [
            (&sidebar, "sidebar", "Documentation"),
            (&primary, "primary", "Sections"),
        ] {
            stream
                .write(q(&self.profile, "navigation", tree.clone()))
                .await?;
            stream.write(q(tree, "type", term("Navigation"))).await?;
            stream
                .write(q(
                    tree,
                    "navigationRegion",
                    Literal::new_simple_literal(region),
                ))
                .await?;
            stream.write(title_quad(tree, title)).await?;
        }
        let mut digest = Sha256::new();
        let mut pending = vec![(self.root.clone(), None)]; // bounded: 512 navigation entries.
        let mut count = 0usize;
        while let Some((directory, parent)) = pending.pop() {
            let document = hints::Directory::read(
                &self.root,
                &directory,
                &self.exclusions,
            )
            .await?;
            if document.hidden {
                continue;
            }
            digest.update(document.source.as_bytes());
            digest.update(document.revision.as_bytes());
            let record = identity("directory-navigation-source", &[
                &self.project,
                &document.source,
            ]);
            for quad in [
                q(
                    &record,
                    "sourcePath",
                    Literal::new_simple_literal(&document.source),
                ),
                q(
                    &record,
                    "contentDigest",
                    Literal::new_simple_literal(&document.revision),
                ),
                q(
                    &record,
                    "sourceMarkup",
                    Literal::new_simple_literal(&document.body),
                ),
            ] {
                stream.write(quad).await?;
            }
            stream
                .write(Quad::new(
                    sidebar.clone(),
                    NamedNode::new_unchecked(
                        "http://www.w3.org/ns/prov#wasDerivedFrom",
                    ),
                    record,
                    GraphName::DefaultGraph,
                ))
                .await?;
            for (order, item) in document.items.into_iter().enumerate() {
                if parent.is_some() &&
                    item.path
                        .file_name()
                        .is_some_and(|name| name == "index.md")
                {
                    continue;
                }
                count += 1;
                ensure!(
                    count <= 512,
                    "directory navigation exceeds 512 entries"
                );
                let relative =
                    super::page::source_path(&self.root, &item.path)?;
                let entry = identity("directory-navigation-entry", &[
                    self.profile.as_str(),
                    &relative,
                ]);
                let metadata = tokio::fs::symlink_metadata(&item.path).await?;
                ensure!(
                    !metadata.is_symlink(),
                    "navigation rejects symlinks"
                );
                let (label, destination) = if metadata.is_dir() {
                    let child = hints::Directory::read(
                        &self.root,
                        &item.path,
                        &self.exclusions,
                    )
                    .await?;
                    if child.hidden {
                        continue;
                    }
                    digest.update(child.revision.as_bytes());
                    let destination = item.path.join("index.md");
                    let destination =
                        if tokio::fs::try_exists(&destination).await? {
                            Some(super::page::source_path(
                                &self.root,
                                &destination,
                            )?)
                        } else {
                            None
                        };
                    let label = item.label.or(child.title).or_else(|| {
                        destination.is_none().then(|| {
                            item.path
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .replace(['-', '_'], " ")
                        })
                    });
                    pending.push((item.path, Some(entry.clone())));
                    (label, destination)
                } else {
                    ensure!(
                        item.path
                            .extension()
                            .is_some_and(|extension| extension == "md"),
                        "navigation destination must be Markdown or a \
                         directory: {relative}"
                    );
                    (item.label, Some(relative.clone()))
                };
                digest.update(relative.as_bytes());
                stream
                    .write(q(&sidebar, "navigationEntry", entry.clone()))
                    .await?;
                if let Some(parent) = &parent {
                    stream
                        .write(q(&entry, "parentEntry", parent.clone()))
                        .await?;
                }
                stream
                    .write(q(&primary, "navigationEntry", entry.clone()))
                    .await?;
                for quad in [
                    q(&entry, "type", term("NavigationEntry")),
                    q(&entry, "order", Literal::from(order as u32)),
                ] {
                    stream.write(quad).await?;
                }
                if let Some(label) = label {
                    stream.write(title_quad(&entry, &label)).await?;
                }
                if let Some(path) = destination {
                    stream
                        .write(q(
                            &entry,
                            "sourcePath",
                            Literal::new_simple_literal(path),
                        ))
                        .await?;
                }
            }
        }
        Ok(format!(
            "directory-navigation-v1:{}",
            hex::encode(digest.finalize())
        ))
    }
}
fn q(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        if predicate == "type" {
            NamedNode::new_unchecked(crate::publication::RDF_TYPE)
        } else {
            term(predicate)
        },
        value,
        GraphName::DefaultGraph,
    )
}
fn title_quad(subject: &NamedNode, title: &str) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        Literal::new_simple_literal(title),
        GraphName::DefaultGraph,
    )
}
