//! A specialized collector for the declarative content of already-published
//! W3C specification HTML. ReSpec authoring JavaScript is not executed.
mod header;
mod parse;
use {
    super::{Collector, SourceIdentity, source::FileInput},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        term,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::path::PathBuf,
    tokio::io::AsyncReadExt,
    url::Url,
};

/// Read one published specification, exposing semantic content and provenance.
pub struct W3cSpecificationFile {
    input: FileInput,
    url:   Url,
}
impl W3cSpecificationFile {
    /// Bind a local, pinned source snapshot to its original publication URL.
    pub fn new(
        project: &str,
        collector: &str,
        path: PathBuf,
        url: &str,
    ) -> Result<Self> {
        let url = Url::parse(url)?;
        ensure!(
            url.scheme() == "https" &&
                url.host_str() == Some("www.w3.org") &&
                url.fragment().is_none(),
            "W3C publication source must be an absolute www.w3.org HTTPS URL \
             without a fragment"
        );
        Ok(Self {
            input: FileInput::new(project, collector, path)?,
            url,
        })
    }

    /// Stable publication target emitted by this collector.
    pub fn target_id(&self) -> NamedNode { self.id("target", "publication") }

    fn id(&self, kind: &str, local: &str) -> NamedNode {
        identity(kind, &[
            self.input.project.as_str(),
            self.input.collector.as_str(),
            self.url.as_str(),
            local,
        ])
    }
}

#[async_trait]
impl Collector for W3cSpecificationFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> { self.input.identity() }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let mut file = tokio::fs::File::open(&self.input.path)
            .await?
            .take(2 * 1024 * 1024 + 1);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        ensure!(
            bytes.len() <= 2 * 1024 * 1024,
            "published W3C source exceeds 2 MiB"
        );
        let revision = format!(
            "w3c-published-v3:{}:{}",
            hex::encode(Sha256::digest(&bytes)),
            self.url
        );
        let source = String::from_utf8(bytes)?;
        let base = self.url.clone();
        let parsed =
            tokio::task::spawn_blocking(move || parse::parse(&source, &base))
                .await??;
        let header = parsed.header;
        let project = &self.input.project;
        let collector = &self.input.collector;
        let contribution = identity("contribution", &[
            project.as_str(),
            collector.as_str(),
            self.input.source()?,
        ]);
        let target = self.target_id();
        let profile = self.id("profile", "publication");
        let scope = self.id("scope", "publication");
        let authority = NamedNode::new_unchecked(
            crate::publication::governance::W3C_AUTHORITY,
        );
        for (subject, property, object) in [
            (project.clone(), "type", term("Project").into()),
            (
                project.clone(),
                "collector",
                collector.clone().into(),
            ),
            (project.clone(), "target", target.clone().into()),
            (project.clone(), "scope", scope.clone().into()),
            (
                collector.clone(),
                "type",
                term("Collector").into(),
            ),
            (
                collector.clone(),
                "kind",
                lit("w3c-published-specification"),
            ),
            (
                collector.clone(),
                "source",
                lit(self.input.source()?),
            ),
            (
                contribution.clone(),
                "type",
                term("Contribution").into(),
            ),
            (
                contribution.clone(),
                "processor",
                collector.clone().into(),
            ),
            (
                contribution.clone(),
                "project",
                project.clone().into(),
            ),
            (
                contribution.clone(),
                "source",
                lit(self.input.source()?),
            ),
            (contribution.clone(), "revision", lit(&revision)),
            (scope.clone(), "type", term("Scope").into()),
            (profile.clone(), "type", term("Profile").into()),
            (target.clone(), "type", term("Target").into()),
            (target.clone(), "scope", scope.clone().into()),
            (target.clone(), "profile", profile.clone().into()),
            (target.clone(), "renderer", lit("specification")),
            (
                target.clone(),
                "destination",
                lit("specification"),
            ),
            (
                target.clone(),
                "publisher",
                authority.clone().into(),
            ),
            (
                target.clone(),
                "publicationStatus",
                lit(&header.status),
            ),
            (
                target.clone(),
                "publicationDate",
                Literal::new_typed_literal(
                    &header.date,
                    NamedNode::new_unchecked(
                        "http://www.w3.org/2001/XMLSchema#date",
                    ),
                )
                .into(),
            ),
            (
                authority.clone(),
                "type",
                NamedNode::new_unchecked(
                    crate::publication::governance::AUTHORITY,
                )
                .into(),
            ),
            (authority.clone(), "shortName", lit("W3C")),
            (
                authority.clone(),
                "url",
                lit("https://www.w3.org/"),
            ),
        ] {
            write(output, &subject, property, object).await?;
        }
        title(output, project, &header.title).await?;
        title(output, &target, &header.title).await?;
        output
            .write(oxrdf::Quad::new(
                authority.clone(),
                NamedNode::new_unchecked(
                    "http://www.w3.org/2000/01/rdf-schema#label",
                ),
                Literal::new_language_tagged_literal(
                    "World Wide Web Consortium",
                    "en",
                )?,
                oxrdf::GraphName::DefaultGraph,
            ))
            .await?;
        output
            .write(oxrdf::Quad::new(
                authority.clone(),
                NamedNode::new_unchecked(
                    crate::publication::governance::PLAYED_BY,
                ),
                NamedNode::new_unchecked(
                    crate::publication::governance::W3C_ORGANIZATION,
                ),
                oxrdf::GraphName::DefaultGraph,
            ))
            .await?;
        if let Some(copyright) = header.copyright {
            write(output, &target, "copyright", lit(&copyright)).await?;
        }
        if let Some(subtitle) = header.subtitle {
            write(output, &target, "subtitle", lit(&subtitle)).await?;
        }
        for (field, url) in header.versions {
            write(
                output,
                &target,
                &field,
                NamedNode::new(url)?.into(),
            )
            .await?;
        }
        for (role, people) in [
            ("editor", header.editors),
            ("author", header.authors),
            ("formerEditor", header.former_editors),
        ] {
            for (index, person) in people.iter().enumerate() {
                let editor = self.id(role, &index.to_string());
                write(output, &target, role, editor.clone().into()).await?;
                title(output, &editor, &person.name).await?;
                write(
                    output,
                    &editor,
                    "order",
                    Literal::from(index as i64).into(),
                )
                .await?;
                if let Some(url) = &person.url {
                    write(output, &editor, "url", lit(url)).await?;
                }
                if !person.note.is_empty() {
                    write(output, &editor, "personNote", lit(&person.note))
                        .await?;
                }
                if let Some(orcid) = &person.orcid {
                    write(
                        output,
                        &editor,
                        "orcid",
                        NamedNode::new(orcid)?.into(),
                    )
                    .await?;
                }
                if let Some(affiliation) = &person.affiliation {
                    write(output, &editor, "affiliation", lit(affiliation))
                        .await?;
                }
            }
        }
        if header.modern {
            for property in ["metadataDisclosure", "tocFrontMatter"] {
                write(
                    output,
                    &profile,
                    property,
                    Literal::from(true).into(),
                )
                .await?;
            }
        }
        if let Some(url) = header.status_url {
            write(
                output,
                &target,
                "statusUrl",
                NamedNode::new(url)?.into(),
            )
            .await?;
        }
        for (index, row) in header.rows.into_iter().enumerate() {
            let id = self.id("metadata-row", &index.to_string());
            write(output, &target, "metadataRow", id.clone().into()).await?;
            write(output, &id, "type", term("MetadataRow").into()).await?;
            write(
                output,
                &id,
                "order",
                Literal::from(index as i64).into(),
            )
            .await?;
            write(output, &id, "field", lit(&row.field)).await?;
            title(output, &id, &row.label).await?;
            if let Some(label) = row.link_label {
                write(output, &id, "linkLabel", lit(&label)).await?;
            }
            if row.field == "custom" {
                write(output, &id, "body", lit(&row.body)).await?;
            }
        }
        for (index, section) in parsed.sections.into_iter().enumerate() {
            let fragment = self.id("fragment", &section.id);
            let component = self.id("component", &section.id);
            let placement = self.id("placement", &section.id);
            let subject = NamedNode::new(
                self.url.join(&format!("#{}", section.id))?.as_str(),
            )?;
            for (subject, property, object) in [
                (scope.clone(), "include", subject.clone().into()),
                (
                    profile.clone(),
                    "placement",
                    placement.clone().into(),
                ),
                (
                    placement.clone(),
                    "type",
                    term("Placement").into(),
                ),
                (
                    placement.clone(),
                    "component",
                    component.clone().into(),
                ),
                (
                    placement.clone(),
                    "order",
                    Literal::from(index as i64).into(),
                ),
                (
                    placement.clone(),
                    "sectionKind",
                    lit(section.kind),
                ),
                (
                    component.clone(),
                    "type",
                    term("Component").into(),
                ),
                (
                    component.clone(),
                    "fragment",
                    fragment.clone().into(),
                ),
                (fragment.clone(), "type", term("Fragment").into()),
                (fragment.clone(), "subject", subject.into()),
                (fragment.clone(), "body", lit(&section.body)),
                (fragment.clone(), "format", lit("text/html")),
                (
                    fragment.clone(),
                    "contribution",
                    contribution.clone().into(),
                ),
                (fragment.clone(), "nativeAnchor", lit(&section.id)),
                (
                    fragment.clone(),
                    "sourceMarkup",
                    lit(&section.original),
                ),
            ] {
                write(output, &subject, property, object).await?;
            }
            title(output, &component, &section.title).await?;
            if let Some(anchor) = section.heading_id {
                write(output, &fragment, "headingAnchor", lit(&anchor)).await?;
            }
            for thing in section.things {
                let node = NamedNode::new(
                    self.url.join(&format!("#{}", thing.id))?.as_str(),
                )?;
                write(output, &node, "type", term(thing.class).into()).await?;
                write(
                    output,
                    &node,
                    "partOf",
                    NamedNode::new(self.url.as_str())?.into(),
                )
                .await?;
                write(output, &node, "fragment", fragment.clone().into())
                    .await?;
                title(output, &node, &thing.title).await?;
            }
        }
        Ok(revision)
    }
}

fn lit(text: &str) -> Term { Literal::new_simple_literal(text).into() }
async fn write(
    output: &mut ContributionStream,
    subject: &NamedNode,
    property: &str,
    object: Term,
) -> Result<()> {
    let predicate = if property == "type" {
        NamedNode::new_unchecked(RDF_TYPE)
    } else {
        term(property)
    };
    output
        .write(Quad::new(
            subject.clone(),
            predicate,
            object,
            GraphName::DefaultGraph,
        ))
        .await
}
async fn title(
    output: &mut ContributionStream,
    subject: &NamedNode,
    text: &str,
) -> Result<()> {
    output
        .write(Quad::new(
            subject.clone(),
            NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            Literal::new_simple_literal(text),
            GraphName::DefaultGraph,
        ))
        .await
}
