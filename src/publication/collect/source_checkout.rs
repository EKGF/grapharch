//! The commit and documentation directory that source links point into.
use {
    super::{Collector, SourceIdentity},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        source_link::{CommitId, RepositoryPath},
        term,
        writer::ContributionStream,
    },
    anyhow::Result,
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};

/// Processor of the recorded source checkout.
const PROCESSOR: &str = "urn:grapharch:source-checkout";

/// Records, for a project declaring `ga:sourceLinkBase`, the commit the build
/// collected and the documentation root's directory in that repository.
pub(crate) struct SourceCheckout {
    project: NamedNode,
    node:    NamedNode,
    commit:  CommitId,
    content: RepositoryPath,
}

impl SourceCheckout {
    pub(crate) fn new(
        project: NamedNode,
        commit: CommitId,
        content: RepositoryPath,
    ) -> Self {
        let node = Self::iri(&project);
        Self { project, node, commit, content }
    }

    /// The record's identity, derived from the project alone so rendering
    /// can find it.
    pub(crate) fn iri(project: &NamedNode) -> NamedNode {
        identity("source-checkout", &[project.as_str()])
    }
}

#[async_trait]
impl Collector for SourceCheckout {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: PROCESSOR,
            source:    self.node.as_str(),
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let mut fields: Vec<(NamedNode, Term)> = vec![
            // bounded: four fixed fields.
            (
                NamedNode::new_unchecked(RDF_TYPE),
                term("SourceCheckout").into(),
            ),
            (term("project"), self.project.clone().into()),
            (
                term("sourceCommit"),
                Literal::new_simple_literal(self.commit.as_str()).into(),
            ),
        ];
        if !self.content.as_str().is_empty() {
            fields.push((
                term("contentDirectory"),
                Literal::new_simple_literal(self.content.as_str()).into(),
            ));
        }
        for (predicate, object) in fields {
            stream
                .write(Quad::new(
                    self.node.clone(),
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(format!(
            "{}:{}",
            self.commit.as_str(),
            self.content.as_str()
        ))
    }
}
