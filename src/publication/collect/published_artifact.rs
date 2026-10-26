//! One generated publication artifact, retained in bounded binary KG chunks.
use {
    super::{Collector, SourceIdentity},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        term,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    base64::{Engine, engine::general_purpose::STANDARD},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
    tokio::io::AsyncReadExt,
};

pub(crate) const MAX_ARTIFACT_BYTES: u64 =
    crate::publication::limits::ARTIFACT_BYTES;
pub(crate) struct PublishedArtifactFile {
    project:   String,
    processor: String,
    path:      PathBuf,
    route:     String,
    id:        NamedNode,
}
impl PublishedArtifactFile {
    pub(crate) fn new(
        project: &str,
        path: PathBuf,
        route: String,
    ) -> Result<Self> {
        ensure!(
            !route.is_empty() &&
                route.split('/').all(|part| {
                    !part.is_empty() &&
                        part != "." &&
                        part != ".." &&
                        part.chars().all(|c| {
                            c.is_ascii_alphanumeric() ||
                                matches!(c, '.' | '-' | '_')
                        })
                }),
            "invalid showcase artifact route"
        );
        Ok(Self {
            project: project.into(),
            processor: identity("published-artifact-collector", &[
                project, &route,
            ])
            .into_string(),
            id: identity("published-artifact", &[project, &route]),
            path,
            route,
        })
    }

    pub(crate) fn id(&self) -> NamedNode { self.id.clone() }

    fn quad(
        &self,
        subject: &NamedNode,
        field: &str,
        object: impl Into<Term>,
    ) -> Quad {
        Quad::new(
            subject.clone(),
            if field == "type" {
                NamedNode::new_unchecked(RDF_TYPE)
            } else {
                term(field)
            },
            object,
            GraphName::DefaultGraph,
        )
    }
}
#[async_trait]
impl Collector for PublishedArtifactFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.route,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        let mut buffer = vec![0u8; 48 * 1024]; // bounded: one base64 content chunk.
        let mut total = 0u64;
        let mut index = 0u32;
        let mut hash = Sha256::new();
        loop {
            let length = file.read(&mut buffer).await?;
            if length == 0 {
                break;
            }
            total += length as u64;
            ensure!(
                total <= MAX_ARTIFACT_BYTES,
                "showcase artifact exceeds {}-byte budget",
                MAX_ARTIFACT_BYTES
            );
            hash.update(&buffer[..length]);
            let chunk = identity("published-artifact-chunk", &[
                self.id.as_str(),
                &index.to_string(),
            ]);
            for quad in [
                self.quad(&self.id, "chunk", chunk.clone()),
                self.quad(&chunk, "order", Literal::from(index)),
                self.quad(
                    &chunk,
                    "base64",
                    Literal::new_simple_literal(
                        STANDARD.encode(&buffer[..length]),
                    ),
                ),
            ] {
                stream.write(quad).await?;
            }
            index += 1;
        }
        let digest = hex::encode(hash.finalize());
        let revision = format!("published-artifact-v2:{}:{digest}", self.route);
        let contribution = identity("contribution", &[
            &self.project,
            &self.processor,
            &self.route,
        ]);
        for quad in [
            self.quad(
                &NamedNode::new(&self.processor)?,
                "type",
                term("Processor"),
            ),
            self.quad(&self.id, "type", term("BinaryAsset")),
            self.quad(
                &self.id,
                "route",
                Literal::new_simple_literal(&self.route),
            ),
            self.quad(
                &self.id,
                "sourcePath",
                Literal::new_simple_literal(&self.route),
            ),
            self.quad(
                &self.id,
                "contentDigest",
                Literal::new_simple_literal(digest),
            ),
            self.quad(
                &self.id,
                "contentSize",
                Literal::from(total as i64),
            ),
            self.quad(&self.id, "contribution", contribution.clone()),
            self.quad(&contribution, "type", term("Contribution")),
            self.quad(
                &contribution,
                "project",
                NamedNode::new(&self.project)?,
            ),
            self.quad(
                &contribution,
                "processor",
                NamedNode::new(&self.processor)?,
            ),
            self.quad(
                &contribution,
                "source",
                Literal::new_simple_literal(&self.route),
            ),
            self.quad(
                &contribution,
                "revision",
                Literal::new_simple_literal(&revision),
            ),
        ] {
            stream.write(quad).await?;
        }
        Ok(revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retained_publications_can_exceed_thirty_two_mib() -> Result<()> {
        use crate::publication::{dataset::Dataset, writer::GraphWriter};
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("large.html");
        std::fs::File::create(&path)?.set_len(33 * 1024 * 1024)?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 2, 1024 * 1024)?;
        let artifact = PublishedArtifactFile::new(
            "urn:large:project",
            path,
            "large.html".into(),
        )?;
        artifact.collect(&writer).await?;
        let snapshot = dataset.snapshot().await?;
        let view =
            crate::publication::view::RevisionView::new(dataset, snapshot);
        assert_eq!(
            view.text(&artifact.id(), "contentSize").await?,
            (33 * 1024 * 1024u64).to_string()
        );
        drop(writer);
        worker.await??;
        Ok(())
    }
}
