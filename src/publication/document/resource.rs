use {
    crate::publication::{
        RDF_TYPE,
        dataset::{Dataset, Stage, identity},
        term,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};

pub(super) struct ResourceDocumentor {
    dataset:   Dataset,
    snapshot:  NamedNode,
    project:   NamedNode,
    class:     NamedNode,
    label:     &'static str,
    processor: NamedNode,
}

impl ResourceDocumentor {
    pub fn new(
        dataset: Dataset,
        snapshot: NamedNode,
        project: &str,
        class: &str,
        label: &'static str,
    ) -> Result<Self> {
        Ok(Self {
            dataset,
            snapshot,
            project: NamedNode::new(project)?,
            class: NamedNode::new(class)?,
            label,
            processor: identity("documentor-v1", &[project, class]),
        })
    }

    pub fn fragment_id(&self, subject: &NamedNode) -> NamedNode {
        identity("generated-description", &[
            self.project.as_str(),
            self.processor.as_str(),
            subject.as_str(),
        ])
    }

    pub async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let pattern = Dataset::logical_pattern(
            &self.snapshot,
            &format!("?subject <{RDF_TYPE}> {}", self.class),
            0,
        );
        let mut subjects = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?subject WHERE {{ {pattern} }}"
            ))
            .await?;
        while let Some(row) = subjects.next().await? {
            let Some(Term::NamedNode(subject)) = row.get("subject") else {
                anyhow::bail!("documentable subjects need stable IRIs")
            };
            self.document_subject(subject, writer).await?;
        }
        self.reconcile_removed_subjects(writer).await?;
        Ok(())
    }

    async fn reconcile_removed_subjects(
        &self,
        writer: &GraphWriter,
    ) -> Result<()> {
        let control = crate::publication::dataset::CONTROL;
        let query = format!(
            "SELECT DISTINCT ?source WHERE {{ GRAPH <{control}> {{ ?owner {} \
             ?version . ?version {} ?attempt . ?attempt {} {} ; {} {} ; {} \
             ?source }} }}",
            term("head"),
            term("data"),
            term("project"),
            self.project,
            term("collector"),
            self.processor,
            term("source")
        );
        let mut previous = self.dataset.select(query).await?;
        while let Some(row) = previous.next().await? {
            let Some(Term::Literal(source)) = row.get("source") else {
                anyhow::bail!("invalid documentor source identity")
            };
            let subject = NamedNode::new(source.value())?;
            let pattern = Dataset::logical_pattern(
                &self.snapshot,
                &format!("{subject} <{RDF_TYPE}> {}", self.class),
                0,
            );
            let mut current = self
                .dataset
                .select(format!(
                    "SELECT ?_owner0 WHERE {{ {pattern} }} LIMIT 1"
                ))
                .await?;
            if current.next().await?.is_none() {
                writer
                    .remove(
                        self.project.as_str(),
                        self.processor.as_str(),
                        source.value(),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn document_subject(
        &self,
        subject: &NamedNode,
        writer: &GraphWriter,
    ) -> Result<()> {
        let mut digest = Sha256::new();
        let pattern = Dataset::logical_pattern(
            &self.snapshot,
            &format!("{subject} ?predicate ?object"),
            0,
        );
        let query = format!(
            "SELECT DISTINCT ?predicate ?object WHERE {{ {pattern} }} ORDER \
             BY ?predicate ?object"
        );
        let rich = super::description::read(
            &crate::publication::view::RevisionView::new(
                self.dataset.clone(),
                self.snapshot.clone(),
            ),
            subject,
            "en",
        )
        .await?;
        let mut rows = self.dataset.select(query.clone()).await?;
        let mut description = rich.clone();
        while let Some(row) = rows.next().await? {
            digest.update(format!(
                "{} {}\n",
                row["predicate"], row["object"]
            ));
            if rich.is_none() &&
                row["predicate"] ==
                    NamedNode::new(
                        "http://www.w3.org/2000/01/rdf-schema#comment",
                    )?
                    .into()
            {
                ensure!(
                    description.is_none(),
                    "{subject} has multiple comments; explicit \
                     language/profile selection is required"
                );
                let Term::Literal(comment) = &row["object"] else {
                    anyhow::bail!("{subject} comment must be a literal")
                };
                description = Some(comment.value().to_owned());
            }
        }
        let revision =
            format!("resource-v2:{}", hex::encode(digest.finalize()));
        if self
            .dataset
            .is_current(
                Stage::Document,
                self.project.as_str(),
                self.processor.as_str(),
                subject.as_str(),
                &revision,
            )
            .await?
        {
            return Ok(());
        }
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                self.processor.as_str(),
                subject.as_str(),
                &revision,
                Stage::Document,
            )
            .await?;
        let contribution = identity("contribution", &[
            self.project.as_str(),
            self.processor.as_str(),
            subject.as_str(),
        ]);
        let description = description
            .unwrap_or_else(|| format!("{subject} is an {}.", self.label));
        self.fragment(
            &mut stream,
            self.fragment_id(subject),
            subject,
            &description,
            "text/markdown",
            &contribution,
        )
        .await?;
        for (predicate, object) in [
            (
                NamedNode::new_unchecked(RDF_TYPE),
                Term::NamedNode(term("Contribution")),
            ),
            (term("processor"), self.processor.clone().into()),
            (term("project"), self.project.clone().into()),
            (
                term("source"),
                Literal::new_simple_literal(subject.as_str()).into(),
            ),
            (
                term("revision"),
                Literal::new_simple_literal(&revision).into(),
            ),
        ] {
            stream
                .write(Quad::new(
                    contribution.clone(),
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        stream
            .write(Quad::new(
                self.processor.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("Documentor"),
                GraphName::DefaultGraph,
            ))
            .await?;
        let mut rows = self.dataset.select(query).await?;
        while let Some(row) = rows.next().await? {
            let assertion = format!(
                "{subject} {} {} .",
                row["predicate"], row["object"]
            );
            let fragment = identity("generated-assertion", &[
                self.project.as_str(),
                self.processor.as_str(),
                &assertion,
            ]);
            self.fragment(
                &mut stream,
                fragment,
                subject,
                &assertion,
                "application/n-triples",
                &contribution,
            )
            .await?;
        }
        stream.complete().await
    }

    async fn fragment(
        &self,
        stream: &mut ContributionStream,
        id: NamedNode,
        subject: &NamedNode,
        body: &str,
        format: &str,
        contribution: &NamedNode,
    ) -> Result<()> {
        for (predicate, object) in [
            (
                NamedNode::new_unchecked(RDF_TYPE),
                Term::NamedNode(term("Fragment")),
            ),
            (term("subject"), subject.clone().into()),
            (
                term("body"),
                Literal::new_simple_literal(body).into(),
            ),
            (
                term("format"),
                Literal::new_simple_literal(format).into(),
            ),
            (term("contribution"), contribution.clone().into()),
        ] {
            stream
                .write(Quad::new(
                    id.clone(),
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(())
    }
}
