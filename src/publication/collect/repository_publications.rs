//! Build-time repository publication discovery, retained as native RDF.
mod api;
#[cfg(test)]
mod tests;
use {
    super::{Collector, SourceIdentity},
    crate::publication::{
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};
const DCT: &str = "http://purl.org/dc/terms/";
const DOAP: &str = "http://usefulinc.com/ns/doap#";
const DCAT: &str = "http://www.w3.org/ns/dcat#";

pub(super) struct RepositoryPublications {
    project:    String,
    processor:  NamedNode,
    source:     String,
    repository: String,
    subject:    NamedNode,
    path:       String,
    hidden:     Vec<String>,
    reserved:   Vec<String>,
    api:        api::Api,
}
impl RepositoryPublications {
    pub async fn configured(
        project: &str,
        processor: &NamedNode,
        source: &str,
        view: &RevisionView,
    ) -> Result<Self> {
        let url = url::Url::parse(source)?;
        ensure!(
            url.scheme() == "https" &&
                url.host_str() == Some("github.com") &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none(),
            "repository publications require a GitHub repository HTTPS URL"
        );
        let repository = url.path().trim_matches('/').to_owned();
        let parts: Vec<_> = repository.split('/').collect(); // bounded: validated owner/name pair.
        ensure!(
            parts.len() == 2 &&
                parts.iter().all(|p| {
                    !p.is_empty() &&
                        p.bytes().all(|c| {
                            c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                        })
                }),
            "repository source must identify owner/name"
        );
        let path = view.text(processor, "publicationPath").await?;
        ensure!(
            !path.is_empty() &&
                path.ends_with('/') &&
                path.split('/').filter(|p| !p.is_empty()).all(|p| {
                    p != "." &&
                        p != ".." &&
                        p.bytes().all(|c| {
                            c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                        })
                }) &&
                !path.starts_with('/'),
            "publicationPath must be a relative URL directory"
        );
        let subject = view.node(processor, "subject").await?;
        let mut hidden = Vec::new(); // bounded: 16 configured prefixes.
        let mut rows = view.objects(processor, "unlistedBranchPrefix").await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(prefix) = &row["value"] else {
                anyhow::bail!("unlistedBranchPrefix must be text")
            };
            ensure!(
                hidden.len() < 16 &&
                    !prefix.value().is_empty() &&
                    prefix.value().len() <= 128 &&
                    !prefix.value().chars().any(char::is_control),
                "invalid unlisted branch prefix"
            );
            hidden.push(prefix.value().into());
        }
        let mut reserved = Vec::new(); // bounded: 32 source-owned canonical versions.
        let mut members = view
            .objects_by(
                &subject,
                &NamedNode::new(format!("{DCT}hasPart"))?,
            )
            .await?;
        while let Some(row) = members.next().await? {
            let Term::NamedNode(member) = &row["value"] else {
                anyhow::bail!("canonical version must have an IRI")
            };
            let Some(Term::Literal(branch)) = view
                .optional_by(member, &NamedNode::new(format!("{DOAP}branch"))?)
                .await?
            else {
                anyhow::bail!("canonical version requires doap:branch")
            };
            ensure!(
                reserved.len() < 32,
                "canonical version budget exceeded"
            );
            reserved.push(branch.value().into());
        }
        Ok(Self {
            project: project.into(),
            processor: processor.clone(),
            source: source.into(),
            repository,
            subject,
            path,
            hidden,
            reserved,
            api: api::Api::environment()?,
        })
    }
}
#[async_trait]
impl Collector for RepositoryPublications {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: self.processor.as_str(),
            source:    &self.source,
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let (versions, revision) = self
            .api
            .observe(
                &self.repository,
                &self.path,
                &self.hidden,
                &self.reserved,
            )
            .await?;
        let observed = chrono::Utc::now().to_rfc3339();
        output
            .write(Quad::new(
                self.subject.clone(),
                NamedNode::new(format!("{DCT}modified"))?,
                Literal::new_typed_literal(
                    &observed,
                    NamedNode::new(
                        "http://www.w3.org/2001/XMLSchema#dateTime",
                    )?,
                ),
                GraphName::DefaultGraph,
            ))
            .await?;
        for version in versions {
            let id = crate::publication::dataset::identity(
                "repository-publication",
                &[self.subject.as_str(), &version.branch],
            );
            output
                .write(Quad::new(
                    self.subject.clone(),
                    NamedNode::new(format!("{DCT}hasPart"))?,
                    id.clone(),
                    GraphName::DefaultGraph,
                ))
                .await?;
            let fields: [(NamedNode, Term); 6] = [
                (
                    NamedNode::new(crate::publication::RDF_TYPE)?,
                    NamedNode::new(format!("{DOAP}Version"))?.into(),
                ),
                (
                    NamedNode::new(format!("{DCT}identifier"))?,
                    Literal::new_simple_literal(version.slug).into(),
                ),
                (
                    NamedNode::new(format!("{DCT}title"))?,
                    Literal::new_simple_literal(version.label).into(),
                ),
                (
                    NamedNode::new(format!("{DOAP}branch"))?,
                    Literal::new_simple_literal(version.branch).into(),
                ),
                (
                    NamedNode::new(format!("{DCAT}accessURL"))?,
                    NamedNode::new(version.url)?.into(),
                ),
                (
                    term("advertised"),
                    Literal::from(version.advertised).into(),
                ),
            ];
            for (predicate, object) in fields {
                output
                    .write(Quad::new(
                        id.clone(),
                        predicate,
                        object,
                        GraphName::DefaultGraph,
                    ))
                    .await?;
            }
        }
        Ok(format!(
            "repository-publications-v1:{revision}:{observed}"
        ))
    }
}
