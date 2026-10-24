//! Explicit execution policy and machine-readable publication diagnostics.
use serde::{Deserialize, Serialize};

/// Whether an operation stops at its first recoverable error.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum FailureMode {
    /// Reject the first error; this is the default.
    #[default]
    FailFast,
    /// Continue supported work and report every encountered error.
    KeepGoing,
}

/// Diagnostic importance, independent of the execution policy.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational observation.
    Info,
    /// Nonfatal concern.
    Warning,
    /// An error that prevents a clean publication.
    Error,
}

/// An error retained with an explicitly requested diagnostic preview.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Importance of this diagnostic.
    pub severity:        Severity,
    /// Stable category for tools and editor integrations.
    pub code:            String,
    /// Human-readable explanation.
    pub message:         String,
    /// Native reference associated with the error.
    pub reference:       String,
    /// Identified fragment containing the error, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fragment: Option<String>,
}

impl Diagnostic {
    /// A documentation fragment usable by HTML, PDF, and diagnostic renderers.
    pub fn markdown(&self) -> String {
        let literal = |value: &str| {
            value
                .lines()
                .map(|line| format!("    {line}\n"))
                .collect::<String>()
        };
        format!(
            "Diagnostic \
             ({:?})\n\nCode:\n\n{}\nMessage:\n\n{}\nReference:\n\n{}",
            self.severity,
            literal(&self.code),
            literal(&self.message),
            literal(&self.reference)
        )
    }
}

pub(crate) async fn record(
    dataset: &super::dataset::Dataset,
    project: &str,
    processor: &str,
    source: &str,
    stage: &str,
    diagnostic: Diagnostic,
) -> anyhow::Result<()> {
    use {
        super::{
            RDF_TYPE,
            dataset::identity,
            diagnostic_graph::{Context, quads},
            term,
        },
        oxrdf::{Literal, NamedNode, Quad, Term},
    };
    let job = uuid::Uuid::new_v4().to_string();
    let graph = NamedNode::new_unchecked("urn:grapharch:diagnostics");
    let context = Context {
        graph:     graph.clone(),
        id:        NamedNode::new(format!("urn:grapharch:operation:{job}"))?,
        project:   NamedNode::new(project)?,
        processor: NamedNode::new(processor)?,
        source:    source.into(),
        about:     identity("source-artifact", &[project, processor, source]),
        revision:  dataset.snapshot_for(project).await?,
    };
    let mut data = quads(&context, &job, &[diagnostic])?;
    let fields: [(NamedNode, Term); 6] = [
        (
            NamedNode::new_unchecked(RDF_TYPE),
            term("Operation").into(),
        ),
        (term("project"), context.project.clone().into()),
        (
            term("source"),
            Literal::new_simple_literal(source).into(),
        ),
        (
            term("processor"),
            context.processor.clone().into(),
        ),
        (
            term("stage"),
            Literal::new_simple_literal(stage).into(),
        ),
        (
            term("status"),
            Literal::new_simple_literal("failed").into(),
        ),
    ];
    for (predicate, object) in fields {
        data.push(Quad::new(
            context.id.clone(),
            predicate,
            object,
            graph.clone(),
        ));
    }
    dataset
        .run(move |store| {
            store.transaction::<_, std::io::Error>(|mut transaction| {
                for quad in &data {
                    transaction.insert(quad.as_ref())?;
                }
                Ok(())
            })?;
            Ok(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_fragments_preserve_untrusted_messages_as_literal_text() {
        let diagnostic = Diagnostic {
            severity:        Severity::Error,
            code:            "parse-failed".into(),
            message:         "<script>unsafe()</script>\n```\n# injected \
                              heading"
                .into(),
            reference:       "`<source>`".into(),
            source_fragment: None,
        };
        let markdown = diagnostic.markdown();
        assert!(markdown.contains("    <script>unsafe()</script>"));
        assert!(markdown.contains("    # injected heading"));
        assert!(markdown.contains("    `<source>`"));
    }
}
