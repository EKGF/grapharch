use {
    super::{MaturityModelDocumentor, model::Model},
    crate::publication::{
        RDF_TYPE,
        dataset::{CONTROL, Stage, identity},
        term,
        writer::GraphWriter,
    },
    anyhow::Result,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};
fn q(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        if predicate == "type" {
            NamedNode::new_unchecked(RDF_TYPE)
        } else {
            term(predicate)
        },
        value,
        GraphName::DefaultGraph,
    )
}
impl MaturityModelDocumentor {
    pub(super) async fn write(
        &self,
        id: &NamedNode,
        body: &str,
        writer: &GraphWriter,
    ) -> Result<()> {
        let revision = format!(
            "maturity-v1:{}",
            hex::encode(Sha256::digest(body.as_bytes()))
        );
        if self
            .dataset
            .is_current(
                Stage::Document,
                self.project.as_str(),
                self.processor.as_str(),
                id.as_str(),
                &revision,
            )
            .await?
        {
            return Ok(());
        }
        let contribution = identity("contribution", &[
            self.project.as_str(),
            self.processor.as_str(),
            id.as_str(),
        ]);
        let fragment = self.fragment_id(id);
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                self.processor.as_str(),
                id.as_str(),
                &revision,
                Stage::Document,
            )
            .await?;
        for quad in [
            q(&self.processor, "type", term("DocumentorUse")),
            q(&contribution, "type", term("Contribution")),
            q(&contribution, "project", self.project.clone()),
            q(&contribution, "processor", self.processor.clone()),
            q(
                &contribution,
                "source",
                Literal::new_simple_literal(id.as_str()),
            ),
            q(
                &contribution,
                "revision",
                Literal::new_simple_literal(revision),
            ),
            q(&fragment, "type", term("Fragment")),
            q(&fragment, "subject", id.clone()),
            q(
                &fragment,
                "format",
                Literal::new_simple_literal("text/markdown"),
            ),
            q(
                &fragment,
                "body",
                Literal::new_simple_literal(body),
            ),
            q(&fragment, "contribution", contribution),
            q(&fragment, "inputRevision", self.snapshot.clone()),
        ] {
            stream.write(quad).await?;
        }
        stream.complete().await?;
        Ok(())
    }

    pub(super) async fn retire(
        &self,
        model: &Model,
        writer: &GraphWriter,
    ) -> Result<()> {
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?source WHERE {{ GRAPH <{CONTROL}> {{ ?owner \
                 {} ?version . ?version {} ?attempt . ?attempt {} {} ; {} {} \
                 ; {} ?source }} }}",
                term("head"),
                term("data"),
                term("project"),
                self.project,
                term("collector"),
                self.processor,
                term("source")
            ))
            .await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(source) = &row["source"] else {
                anyhow::bail!("invalid maturity contribution source")
            };
            if !model.nodes.contains_key(&NamedNode::new(source.value())?) {
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
}
