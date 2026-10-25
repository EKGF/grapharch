use {
    crate::publication::{dataset::identity, term},
    anyhow::{Context, Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
    tokio::{fs::File, io::AsyncReadExt},
};

/// Maximum authored file size. Large corpora consist of independently processed
/// fragments.
pub const MAX_AUTHORED_BYTES: u64 = 256 * 1024;
pub const MAX_RDF_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct FileInput {
    pub project:   NamedNode,
    pub collector: NamedNode,
    pub path:      PathBuf,
}

impl FileInput {
    pub fn new(project: &str, collector: &str, path: PathBuf) -> Result<Self> {
        Ok(Self {
            project: NamedNode::new(project)?,
            collector: NamedNode::new(collector)?,
            path,
        })
    }

    pub fn source(&self) -> Result<&str> {
        self.path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "source path is not UTF-8: {}",
                self.path.display()
            )
        })
    }

    pub fn identity(&self) -> Result<super::SourceIdentity<'_>> {
        Ok(super::SourceIdentity {
            project:   self.project.as_str(),
            processor: self.collector.as_str(),
            source:    self.source()?,
        })
    }

    pub fn fragment_id(&self) -> NamedNode {
        identity("fragment", &[
            self.project.as_str(),
            self.collector.as_str(),
            &self.path.to_string_lossy(),
        ])
    }

    pub async fn authored(&self) -> Result<(String, String)> {
        let mut file =
            File::open(&self.path).await?.take(MAX_AUTHORED_BYTES + 1);
        // bounded: a single authored fragment file, at most MAX_AUTHORED_BYTES
        // + 1.
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        ensure!(
            bytes.len() as u64 <= MAX_AUTHORED_BYTES,
            "{} exceeds authored file limit",
            self.path.display()
        );
        let revision = format!("authored-v2:{:x}", Sha256::digest(&bytes));
        Ok((
            String::from_utf8(bytes)
                .context("authored source must be UTF-8")?,
            revision,
        ))
    }

    pub fn description(
        &self,
        body: &str,
        format: &str,
        subject: Option<NamedNode>,
        title: Option<&str>,
        revision: &str,
    ) -> Result<Vec<Quad>> {
        let fragment = self.fragment_id();
        let contribution = identity("contribution", &[
            self.project.as_str(),
            self.collector.as_str(),
            self.source()?,
        ]);
        // bounded: fixed metadata plus one capped authored body literal.
        let mut quads = vec![
            self.quad(&self.collector, "type", term("Processor")),
            self.quad(&fragment, "type", term("Fragment")),
            self.quad(
                &fragment,
                "body",
                Literal::new_simple_literal(body),
            ),
            self.quad(
                &fragment,
                "format",
                Literal::new_simple_literal(format),
            ),
            self.quad(&fragment, "contribution", contribution.clone()),
            self.quad(&contribution, "type", term("Contribution")),
            self.quad(&contribution, "project", self.project.clone()),
            self.quad(
                &contribution,
                "source",
                Literal::new_simple_literal(self.source()?),
            ),
            self.quad(&contribution, "processor", self.collector.clone()),
            self.quad(
                &contribution,
                "revision",
                Literal::new_simple_literal(revision),
            ),
        ];
        let subject = subject.unwrap_or_else(|| {
            identity("source-artifact", &[
                self.project.as_str(),
                self.collector.as_str(),
                &self.path.to_string_lossy(),
            ])
        });
        quads.push(self.quad(&fragment, "subject", subject));
        if let Some(title) = title {
            quads.push(Quad::new(
                fragment,
                NamedNode::new("http://purl.org/dc/terms/title")?,
                Literal::new_simple_literal(title),
                GraphName::DefaultGraph,
            ));
        }
        Ok(quads)
    }

    pub fn quad(
        &self,
        subject: &NamedNode,
        predicate: &str,
        object: impl Into<oxrdf::Term>,
    ) -> Quad {
        let predicate = if predicate == "type" {
            NamedNode::new_unchecked(crate::publication::RDF_TYPE)
        } else {
            term(predicate)
        };
        Quad::new(
            subject.clone(),
            predicate,
            object,
            GraphName::DefaultGraph,
        )
    }
}
