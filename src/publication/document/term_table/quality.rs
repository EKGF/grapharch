//! Missing English labels and definitions of documented terms remain KG
//! diagnostics, whether or not a publication shows a quality appendix.
use {
    super::{documentor::TermTableDocumentor, row::TermRow},
    crate::publication::{
        dataset::identity,
        diagnostic_graph,
        diagnostics::{Diagnostic, Severity},
        document::description::missing_annotations,
        term,
        writer::ContributionStream,
    },
    anyhow::Result,
    oxrdf::NamedNode,
};

impl TermTableDocumentor {
    pub(super) async fn diagnostics(
        &self,
        stream: &mut ContributionStream,
        rows: &[(usize, TermRow)],
        context: &diagnostic_graph::Context,
        revision: &str,
    ) -> Result<()> {
        for (_, row) in rows {
            if !self.terms.contains(&row.subject) {
                continue;
            }
            for (missing, code, label, definition) in [
                (
                    row.missing_label,
                    "missing-source-label",
                    true,
                    false,
                ),
                (
                    row.missing_definition,
                    "missing-source-definition",
                    false,
                    true,
                ),
            ] {
                if !missing {
                    continue;
                }
                let diagnostic = Diagnostic {
                    severity:        Severity::Warning,
                    code:            code.into(),
                    message:         missing_annotations(
                        label, definition, "en",
                    )
                    .expect("a missing annotation has a message"),
                    reference:       row.subject.as_str().into(),
                    source_fragment: self
                        .fragments()
                        .details
                        .map(NamedNode::into_string),
                };
                let job = identity("source-quality", &[
                    row.subject.as_str(),
                    code,
                    revision,
                ]);
                let context = diagnostic_graph::Context {
                    about: row.subject.clone(),
                    ..context.clone()
                };
                for quad in diagnostic_graph::quads(&context, job.as_str(), &[
                    diagnostic,
                ])?
                .into_iter()
                .filter(|quad| quad.predicate != term("diagnosticCount"))
                {
                    stream.write(quad).await?;
                }
            }
        }
        Ok(())
    }
}
