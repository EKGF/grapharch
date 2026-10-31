//! A bounded first-parent history and exact tag associations observed by gix.
use {
    super::{Collector, Revision, SourceIdentity, identity},
    crate::publication::{RDF_TYPE, writer::ContributionStream},
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};
const PROV: &str = "http://www.w3.org/ns/prov#";
const DCT: &str = "http://purl.org/dc/terms/";
#[derive(Clone)]
pub(super) struct Commit {
    id:        String,
    author:    (String, String, i64),
    committer: (String, String, i64),
}

pub(super) fn history(
    repo: &gix::Repository,
    tip: &gix::Commit<'_>,
) -> Result<Vec<Commit>> {
    let mut history = Vec::new(); // bounded: at most five first-parent commits.
    let shallow = repo.shallow_commits()?;
    let mut current = Some(tip.id);
    while let Some(id) = current.take() {
        let commit = repo.find_commit(id)?;
        let a = commit.author()?;
        let c = commit.committer()?;
        history.push(Commit {
            id:        id.to_string(),
            author:    (
                a.name.to_string(),
                a.email.to_string(),
                a.time()?.seconds,
            ),
            committer: (
                c.name.to_string(),
                c.email.to_string(),
                c.time()?.seconds,
            ),
        });
        if history.len() == 5 ||
            shallow
                .as_ref()
                .is_some_and(|commits| commits.contains(&id))
        {
            break;
        }
        current = commit.parent_ids().next().map(|id| id.detach());
    }
    Ok(history)
}

pub(super) fn tags(
    repo: &gix::Repository,
    tip: &gix::Commit<'_>,
) -> Result<Vec<String>> {
    let mut tags = Vec::new(); // bounded: at most 64 names attached to this exact commit.
    let refs = repo.references()?;
    let mut examined = 0;
    for reference in refs.tags()? {
        examined += 1;
        ensure!(
            examined <= 100_000,
            "Git tag discovery exceeds reference limit"
        );
        let mut reference = reference?;
        let name = reference.name().as_bstr().to_string();
        let object = reference.peel_to_id()?.object()?;
        if object.kind == gix::objs::Kind::Commit && object.id == tip.id {
            ensure!(tags.len() < 64, "Git commit exceeds tag limit");
            tags.push(
                name.strip_prefix("refs/tags/")
                    .context("tag reference outside tags namespace")?
                    .to_owned(),
            );
        }
    }
    tags.sort();
    Ok(tags)
}
fn quad(
    subject: NamedNode,
    predicate: &str,
    object: impl Into<Term>,
) -> Result<Quad> {
    Ok(Quad::new(
        subject,
        NamedNode::new(predicate)?,
        object,
        GraphName::DefaultGraph,
    ))
}

#[async_trait]
impl Collector for Revision {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: "urn:grapharch:git-provenance",
            source:    self.local_location.as_deref().unwrap_or(&self.origin),
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let repository = identity("git-repository", &[
            &self.origin,
            self.local_location.as_deref().unwrap_or(""),
        ]);
        for assertion in [
            quad(
                repository.clone(),
                RDF_TYPE,
                NamedNode::new("https://schema.org/SoftwareSourceCode")?,
            )?,
            quad(
                repository.clone(),
                &format!("{DCT}identifier"),
                Literal::new_simple_literal(&self.origin),
            )?,
            quad(
                repository.clone(),
                "https://schema.org/codeRepository",
                NamedNode::new(&self.origin)?,
            )?,
            quad(
                repository.clone(),
                &format!("{PROV}hadMember"),
                self.entity(),
            )?,
        ] {
            stream.write(assertion).await?;
        }
        if let Some(location) = &self.local_location {
            stream
                .write(quad(
                    repository.clone(),
                    &format!("{PROV}atLocation"),
                    NamedNode::new(location)?,
                )?)
                .await?;
        }
        for tag in &self.tags {
            stream
                .write(quad(
                    repository.clone(),
                    "https://schema.org/version",
                    Literal::new_simple_literal(tag),
                )?)
                .await?;
        }
        for (index, record) in self.history.iter().enumerate() {
            let commit = identity("git-commit", &[&record.id]);
            for assertion in [
                quad(
                    commit.clone(),
                    RDF_TYPE,
                    NamedNode::new(format!("{PROV}Entity"))?,
                )?,
                quad(
                    commit.clone(),
                    &format!("{DCT}identifier"),
                    Literal::new_simple_literal(&record.id),
                )?,
                quad(
                    commit.clone(),
                    &format!("{PROV}wasDerivedFrom"),
                    repository.clone(),
                )?,
            ] {
                stream.write(assertion).await?;
            }
            if let Some(parent) = self.history.get(index + 1) {
                stream
                    .write(quad(
                        commit.clone(),
                        &format!("{PROV}wasRevisionOf"),
                        identity("git-commit", &[&parent.id]),
                    )?)
                    .await?;
            }
            for (role, signature) in
                [("author", &record.author), ("committer", &record.committer)]
            {
                let agent =
                    identity("git-agent", &[&signature.0, &signature.1]);
                let attribution =
                    identity("git-attribution", &[&record.id, role]);
                let role_iri =
                    NamedNode::new(format!("urn:grapharch:git-role:{role}"))?;
                let time = chrono::DateTime::from_timestamp(signature.2, 0)
                    .context("invalid Git timestamp")?
                    .to_rfc3339();
                for assertion in [
                    quad(
                        agent.clone(),
                        RDF_TYPE,
                        NamedNode::new(format!("{PROV}Person"))?,
                    )?,
                    quad(
                        attribution.clone(),
                        RDF_TYPE,
                        NamedNode::new(format!("{PROV}Attribution"))?,
                    )?,
                    quad(
                        role_iri.clone(),
                        RDF_TYPE,
                        NamedNode::new(format!("{PROV}Role"))?,
                    )?,
                    quad(
                        role_iri.clone(),
                        "http://www.w3.org/2000/01/rdf-schema#label",
                        Literal::new_simple_literal(role),
                    )?,
                    quad(
                        commit.clone(),
                        &format!("{PROV}qualifiedAttribution"),
                        attribution.clone(),
                    )?,
                    quad(
                        attribution.clone(),
                        &format!("{PROV}agent"),
                        agent.clone(),
                    )?,
                    quad(attribution, &format!("{PROV}hadRole"), role_iri)?,
                    quad(
                        agent.clone(),
                        "http://xmlns.com/foaf/0.1/name",
                        Literal::new_simple_literal(&signature.0),
                    )?,
                    quad(
                        agent,
                        "https://schema.org/email",
                        Literal::new_simple_literal(&signature.1),
                    )?,
                    quad(
                        commit.clone(),
                        &format!(
                            "{}{}",
                            if role == "author" { DCT } else { PROV },
                            if role == "author" {
                                "created"
                            } else {
                                "generatedAtTime"
                            }
                        ),
                        Literal::new_typed_literal(
                            time,
                            NamedNode::new(
                                "http://www.w3.org/2001/XMLSchema#dateTime",
                            )?,
                        ),
                    )?,
                ] {
                    stream.write(assertion).await?;
                }
            }
        }
        if self.working_tree {
            let working = identity("git-working-tree", &[&self.origin]);
            stream
                .write(quad(
                    working.clone(),
                    &format!("{PROV}wasDerivedFrom"),
                    self.entity(),
                )?)
                .await?;
            stream
                .write(quad(
                    working,
                    "http://www.w3.org/2000/01/rdf-schema#label",
                    Literal::new_simple_literal(
                        "Working tree observed against HEAD; uncommitted \
                         files are collected independently",
                    ),
                )?)
                .await?;
        }
        // Tags can move independently of the commit; include all observed
        // metadata in the source revision so an unchanged commit cannot
        // reuse stale tags.
        let metadata = format!(
            "{}:{}:{:?}:{:?}",
            self.commit, self.working_tree, self.tags, self.local_location
        );
        Ok(identity("git-provenance-v3", &[&metadata]).into_string())
    }
}
