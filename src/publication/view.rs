//! Typed, streaming access to the logical assertions of one pinned revision.

use {
    super::{
        RDF_TYPE,
        dataset::{Dataset, Rows},
        term,
        validation::rules,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

/// A read-only view that cannot expose private or superseded contribution
/// graphs.
#[derive(Clone)]
pub struct RevisionView {
    dataset:  Dataset,
    revision: NamedNode,
}

impl RevisionView {
    /// Bind all reads to one snapshot.
    pub fn new(dataset: Dataset, revision: NamedNode) -> Self {
        Self { dataset, revision }
    }

    /// Stream distinct values for a normalized publication predicate.
    pub async fn objects(
        &self,
        subject: &NamedNode,
        property: &str,
    ) -> Result<Rows> {
        self.objects_by(subject, &term(property)).await
    }

    /// Stream values for a fully qualified RDF predicate, including standard
    /// metadata.
    pub async fn objects_by(
        &self,
        subject: &NamedNode,
        predicate: &NamedNode,
    ) -> Result<Rows> {
        let pattern = self.pattern(&format!("{subject} {predicate} ?value"), 0);
        self.dataset
            .select(format!(
                "SELECT DISTINCT ?value WHERE {{ {pattern} }}"
            ))
            .await
    }

    /// Read an optional singleton, rejecting ambiguity across contributions.
    pub async fn optional(
        &self,
        subject: &NamedNode,
        property: &str,
    ) -> Result<Option<Term>> {
        self.optional_by(subject, &term(property)).await
    }

    /// Read an optional singleton for a fully qualified RDF predicate.
    pub async fn optional_by(
        &self,
        subject: &NamedNode,
        predicate: &NamedNode,
    ) -> Result<Option<Term>> {
        let mut rows = self.objects_by(subject, predicate).await?;
        let value = rows.next().await?.map(|row| row["value"].clone());
        ensure!(
            rows.next().await?.is_none(),
            "{subject} conflicting {predicate}"
        );
        Ok(value)
    }

    /// Read a required IRI reference.
    pub async fn node(
        &self,
        subject: &NamedNode,
        property: &str,
    ) -> Result<NamedNode> {
        match self.optional(subject, property).await? {
            Some(Term::NamedNode(node)) => Ok(node),
            _ => bail!("{subject} missing or invalid {property} IRI"),
        }
    }

    /// Read a required literal without RDF serialization quotes.
    pub async fn text(
        &self,
        subject: &NamedNode,
        property: &str,
    ) -> Result<String> {
        match self.optional(subject, property).await? {
            Some(Term::Literal(text)) => Ok(text.value().to_owned()),
            _ => bail!("{subject} missing or invalid {property} literal"),
        }
    }

    /// Determine whether an explicitly declared type exists in this revision.
    pub async fn has_type(
        &self,
        subject: &NamedNode,
        class: &str,
    ) -> Result<bool> {
        let classes = if class == "Processor" {
            format!(
                "VALUES ?class {{ {} {} {} }}",
                term("Processor"),
                term("Collector"),
                term("Documentor")
            )
        } else if class == "Authority" {
            format!(
                "VALUES ?class {{ {} <{}> }}",
                term(class),
                super::governance::AUTHORITY
            )
        } else {
            format!("VALUES ?class {{ {} }}", term(class))
        };
        let pattern =
            self.pattern(&format!("{subject} <{RDF_TYPE}> ?class"), 0);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT ?class WHERE {{ {classes} {pattern} }} LIMIT 1"
            ))
            .await?;
        Ok(rows.next().await?.is_some())
    }

    /// Validate logical definitions using the same field contracts as
    /// standalone validation.
    pub async fn validate(&self) -> Result<()> {
        for (class, fields) in rules() {
            let pattern = self.pattern(
                &format!("?subject <{RDF_TYPE}> {}", term(class)),
                0,
            );
            let mut subjects = self
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?subject WHERE {{ {pattern} }}"
                ))
                .await?;
            while let Some(row) = subjects.next().await? {
                let Term::NamedNode(subject) = &row["subject"] else {
                    bail!("{class} requires a stable IRI")
                };
                for field in &fields {
                    let mut rows = self.objects(subject, field.name).await?;
                    let mut count = 0u64;
                    while let Some(row) = rows.next().await? {
                        count += 1;
                        let value = &row["value"];
                        if field.literal {
                            field.check_literal(value, subject)?;
                        } else {
                            let Term::NamedNode(node) = value else {
                                bail!("{subject} {} must be an IRI", field.name)
                            };
                            if let Some(reference) = field.reference {
                                ensure!(
                                    self.has_type(node, reference).await?,
                                    "{subject} {} references undefined \
                                     {reference}: {node}",
                                    field.name
                                );
                            }
                        }
                    }
                    ensure!(
                        !field.required || count > 0,
                        "{subject} missing {}",
                        field.name
                    );
                    ensure!(
                        field.multiple || count <= 1,
                        "{subject} conflicting {}",
                        field.name
                    );
                }
                if class == "Target" {
                    ensure!(
                        crate::publication::Renderer::parse(
                            &self.text(subject, "renderer").await?
                        )
                        .is_ok(),
                        "{subject} unsupported renderer"
                    );
                    ensure!(
                        !self.text(subject, "destination").await?.is_empty(),
                        "{subject} empty destination"
                    );
                }
                if class == "Scope" {
                    let depth = self.optional(subject, "maxDepth").await?;
                    let mut traversals =
                        self.objects(subject, "traverse").await?;
                    ensure!(
                        depth.is_some() || traversals.next().await?.is_none(),
                        "{subject} traverse requires maxDepth"
                    );
                    if let Some(depth) = depth {
                        super::validation::unsigned(&depth, "scope maxDepth")?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn pattern(&self, pattern: &str, index: u16) -> String {
        Dataset::logical_pattern(&self.revision, pattern, index)
    }
}
