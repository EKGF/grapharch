//! The repository facts as RDF in the documentation knowledge graph.
//!
//! `<ga:Project> ga:softwareProject <doap:Project>`; the software project
//! carries DOAP name, descriptions, homepage, SPDX license, languages,
//! repository, bug database and releases, schema.org keywords and discussion
//! URL, and one `ga:CodePackage` per manifest package.
use {
    super::{
        RepositoryFacts,
        facts::RepositoryDiagnostic,
        package::CodePackage,
    },
    crate::publication::{
        RDF_TYPE,
        collect::{Collector, SourceIdentity},
        dataset::identity,
        term,
        writer::ContributionStream,
    },
    anyhow::Result,
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};

const DOAP: &str = "http://usefulinc.com/ns/doap#";
const SCHEMA: &str = "https://schema.org/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const PROCESSOR: &str = "urn:grapharch:repository-discovery";

fn doap(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{DOAP}{local}"))
}
fn schema(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{SCHEMA}{local}"))
}
fn typed(value: impl Into<String>, datatype: &str) -> Term {
    Literal::new_typed_literal(
        value,
        NamedNode::new_unchecked(format!("{XSD}{datatype}")),
    )
    .into()
}
fn text(value: &str) -> Term { Literal::new_simple_literal(value).into() }
fn iri(url: &url::Url) -> Term { NamedNode::new_unchecked(url.as_str()).into() }

/// Collects one repository's facts for one documentation project.
pub(crate) struct RepositoryRecord {
    project: NamedNode,
    source:  String,
    facts:   RepositoryFacts,
}

impl RepositoryRecord {
    pub(crate) fn new(
        project: NamedNode,
        source: String,
        facts: RepositoryFacts,
    ) -> Self {
        Self { project, source, facts }
    }

    /// The software project documented by `project`.
    pub(crate) fn software(project: &NamedNode) -> NamedNode {
        identity("software-project", &[project.as_str()])
    }

    fn quads(&self) -> Vec<Quad> {
        let mut out = Vec::new(); // bounded: fields of one repository and its packages.
        let mut put =
            |subject: &NamedNode, predicate: NamedNode, object: Term| {
                out.push(Quad::new(
                    subject.clone(),
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ));
            };
        let rdf_type = NamedNode::new_unchecked(RDF_TYPE);
        let facts = &self.facts;
        let software = Self::software(&self.project);
        put(
            &self.project,
            term("softwareProject"),
            software.clone().into(),
        );
        put(
            &software,
            rdf_type.clone(),
            doap("Project").into(),
        );
        if let Some(name) = facts.name() {
            put(&software, doap("name"), text(&name));
        }
        if let Some(description) = facts.description() {
            put(&software, doap("shortdesc"), text(&description));
        }
        if let Some(homepage) = facts.homepage() {
            put(&software, doap("homepage"), iri(&homepage));
        }
        if let Some(version) = facts.version() {
            put(
                &software,
                schema("softwareVersion"),
                text(version.as_str()),
            );
        }
        if let Some(license) = facts.license() {
            put(
                &software,
                term("licenseExpression"),
                text(license.as_str()),
            );
            if let Some(license) = license.license_iri() {
                put(&software, doap("license"), iri(&license));
            }
        }
        let mut languages: Vec<&str> = facts
            .manifests
            .packages()
            .iter()
            .map(|p| p.ecosystem.language())
            .collect();
        languages.sort_unstable();
        languages.dedup();
        for language in languages {
            put(
                &software,
                doap("programming-language"),
                text(language),
            );
        }
        let repository = identity("software-repository", &[software.as_str()]);
        if facts.forge.is_some() || facts.git.is_some() {
            put(
                &software,
                doap("repository"),
                repository.clone().into(),
            );
            put(
                &repository,
                rdf_type.clone(),
                doap("GitRepository").into(),
            );
        }
        if let Some(forge) = &facts.forge {
            put(&repository, doap("browse"), iri(&forge.web_url()));
            put(
                &repository,
                doap("location"),
                iri(&forge.web_url()),
            );
            put(
                &repository,
                term("forge"),
                term(forge.kind().label()).into(),
            );
            put(
                &software,
                doap("bug-database"),
                iri(&forge.issues_url()),
            );
            if facts.metadata.as_ref().is_some_and(|m| m.discussions) &&
                let Some(url) = forge.discussions_url()
            {
                put(&software, schema("discussionUrl"), iri(&url));
            }
        }
        if let Some(git) = &facts.git {
            put(
                &repository,
                term("headCommit"),
                text(git.head.as_str()),
            );
            if let Some(branch) = &git.default_branch {
                put(
                    &repository,
                    term("defaultBranch"),
                    text(branch.as_str()),
                );
            }
            put(
                &repository,
                term("workingTreeDirty"),
                typed(git.dirty.to_string(), "boolean"),
            );
            put(
                &repository,
                schema("dateCreated"),
                typed(git.first_commit.to_rfc3339(), "dateTime"),
            );
            put(
                &repository,
                schema("dateModified"),
                typed(git.last_commit.to_rfc3339(), "dateTime"),
            );
            if let Some(tag) = &git.latest_tag {
                put(
                    &repository,
                    term("latestTag"),
                    text(tag.name.as_str()),
                );
            }
        }
        if let Some(metadata) = &facts.metadata {
            for topic in &metadata.topics {
                put(&software, schema("keywords"), text(topic));
            }
            if let Some(visibility) = metadata.visibility {
                put(
                    &repository,
                    term("repositoryVisibility"),
                    term(visibility.term()).into(),
                );
            }
            put(
                &repository,
                term("archived"),
                typed(metadata.archived.to_string(), "boolean"),
            );
            if let Some(stars) = metadata.stars {
                put(
                    &repository,
                    term("starCount"),
                    typed(stars.to_string(), "nonNegativeInteger"),
                );
            }
            if let Some(release) = &metadata.latest_release {
                let node = identity("software-release", &[
                    software.as_str(),
                    &release.tag,
                ]);
                put(&software, doap("release"), node.clone().into());
                put(&node, rdf_type.clone(), doap("Version").into());
                put(&node, doap("revision"), text(&release.tag));
                if let Some(name) = &release.name {
                    put(&node, doap("name"), text(name));
                }
                if let Some(date) = release.date {
                    put(
                        &node,
                        doap("created"),
                        typed(date.to_rfc3339(), "dateTime"),
                    );
                }
                if let Some((_, Some(notes))) = facts.release() {
                    put(&node, term("releaseNotes"), iri(&notes));
                }
            }
        }
        let primary = facts.primary().map(|p| p.manifest.clone());
        for package in facts.manifests.packages() {
            let node = Self::package(&software, package);
            put(
                &software,
                term("codePackage"),
                node.clone().into(),
            );
            if primary.as_deref() == Some(package.manifest.as_str()) {
                put(
                    &software,
                    term("primaryPackage"),
                    node.clone().into(),
                );
            }
            package_quads(&mut put, &node, package);
        }
        for (index, diagnostic) in facts.diagnostics.iter().enumerate() {
            diagnostic_quads(&mut put, &software, index, diagnostic);
        }
        out
    }

    fn package(software: &NamedNode, package: &CodePackage) -> NamedNode {
        identity("code-package", &[
            software.as_str(),
            &package.manifest,
        ])
    }
}

fn package_quads(
    put: &mut impl FnMut(&NamedNode, NamedNode, Term),
    node: &NamedNode,
    package: &CodePackage,
) {
    put(
        node,
        NamedNode::new_unchecked(RDF_TYPE),
        term("CodePackage").into(),
    );
    put(node, doap("name"), text(package.name.as_str()));
    put(
        node,
        term("ecosystem"),
        term(package.ecosystem.term()).into(),
    );
    put(
        node,
        term("manifestPath"),
        text(&package.manifest),
    );
    if let Some(version) = &package.version {
        put(
            node,
            schema("softwareVersion"),
            text(version.as_str()),
        );
    }
    if let Some(description) = &package.description {
        put(node, doap("shortdesc"), text(description));
    }
    if let Some(license) = &package.license {
        put(
            node,
            term("licenseExpression"),
            text(license.as_str()),
        );
    }
    for (predicate, url) in [
        (schema("codeRepository"), &package.repository),
        (doap("homepage"), &package.homepage),
        (term("registryPage"), &package.registry_page()),
        (term("apiReference"), &package.api_reference()),
    ] {
        if let Some(url) = url {
            put(node, predicate, iri(url));
        }
    }
    for keyword in &package.keywords {
        put(node, schema("keywords"), text(keyword));
    }
    for category in &package.categories {
        put(
            node,
            schema("applicationCategory"),
            text(category),
        );
    }
}

fn diagnostic_quads(
    put: &mut impl FnMut(&NamedNode, NamedNode, Term),
    software: &NamedNode,
    index: usize,
    diagnostic: &RepositoryDiagnostic,
) {
    let node = identity("repository-diagnostic", &[
        software.as_str(),
        &index.to_string(),
    ]);
    put(
        &node,
        NamedNode::new_unchecked(RDF_TYPE),
        term("Diagnostic").into(),
    );
    put(&node, term("about"), software.clone().into());
    put(&node, term("severity"), text("warning"));
    put(
        &node,
        term("code"),
        text(diagnostic.code.as_str()),
    );
    put(&node, term("message"), text(&diagnostic.message));
}

#[async_trait]
impl Collector for RepositoryRecord {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: PROCESSOR,
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let mut hash = Sha256::new();
        for quad in self.quads() {
            hash.update(quad.to_string().as_bytes());
            stream.write(quad).await?;
        }
        Ok(format!(
            "repository-v1:{}",
            hex::encode(hash.finalize())
        ))
    }
}
