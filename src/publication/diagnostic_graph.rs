use {
    crate::publication::{RDF_TYPE, diagnostics::Diagnostic, term},
    anyhow::Result,
    oxrdf::{Literal, NamedNode, Quad, Term},
};

pub(crate) struct Context {
    pub graph:     NamedNode,
    pub id:        NamedNode,
    pub project:   NamedNode,
    pub about:     NamedNode,
    pub revision:  NamedNode,
    pub processor: NamedNode,
    pub source:    String,
}

pub(crate) fn quads(
    context: &Context,
    job: &str,
    errors: &[Diagnostic],
) -> Result<Vec<Quad>> {
    let graph = context.graph.clone();
    let job_id = context.id.clone();
    let contribution =
        NamedNode::new(format!("urn:grapharch:diagnostics:{job}"))?;
    let processor = context.processor.clone();
    // bounded: publication diagnostics have an explicit 1024-entry budget.
    anyhow::ensure!(
        errors.len() <= 1024,
        "publication diagnostic budget exceeded"
    );
    let mut quads = Vec::new();
    let mut put = |subject: &NamedNode, field: &str, object: Term| {
        let predicate = if field == "type" {
            NamedNode::new_unchecked(RDF_TYPE)
        } else {
            term(field)
        };
        quads.push(Quad::new(
            subject.clone(),
            predicate,
            object,
            graph.clone(),
        ));
    };
    put(
        &job_id,
        "diagnosticCount",
        Literal::from(errors.len() as i64).into(),
    );
    if !errors.is_empty() {
        put(&contribution, "type", term("Contribution").into());
        put(
            &contribution,
            "processor",
            processor.clone().into(),
        );
        put(&processor, "type", term("Processor").into());
        put(
            &contribution,
            "project",
            context.project.clone().into(),
        );
        put(
            &contribution,
            "source",
            Literal::new_simple_literal(context.source.as_str()).into(),
        );
        put(
            &contribution,
            "revision",
            Literal::new_simple_literal(job).into(),
        );
        put(
            &contribution,
            "stage",
            Literal::new_simple_literal("diagnose").into(),
        );
    }
    for (index, error) in errors.iter().enumerate() {
        let id =
            NamedNode::new(format!("urn:grapharch:diagnostic:{job}:{index}"))?;
        let fragment = NamedNode::new(format!(
            "urn:grapharch:diagnostic-fragment:{job}:{index}"
        ))?;
        put(&job_id, "diagnostic", id.clone().into());
        put(&id, "type", term("Diagnostic").into());
        put(
            &id,
            "severity",
            Literal::new_simple_literal(
                format!("{:?}", error.severity).to_lowercase(),
            )
            .into(),
        );
        put(
            &id,
            "code",
            Literal::new_simple_literal(&error.code).into(),
        );
        put(
            &id,
            "message",
            Literal::new_simple_literal(&error.message).into(),
        );
        put(
            &id,
            "unresolvedReference",
            Literal::new_simple_literal(&error.reference).into(),
        );
        put(&id, "about", context.about.clone().into());
        put(
            &id,
            "inputRevision",
            context.revision.clone().into(),
        );
        put(&id, "fragment", fragment.clone().into());
        if let Some(source) = &error.source_fragment {
            put(
                &id,
                "sourceFragment",
                NamedNode::new(source)?.into(),
            );
        }
        put(&fragment, "type", term("Fragment").into());
        put(&fragment, "subject", id.into());
        put(
            &fragment,
            "format",
            Literal::new_simple_literal("text/markdown").into(),
        );
        put(
            &fragment,
            "body",
            Literal::new_simple_literal(error.markdown()).into(),
        );
        put(
            &fragment,
            "contribution",
            contribution.clone().into(),
        );
    }
    Ok(quads)
}
