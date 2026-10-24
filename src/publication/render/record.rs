use {
    super::{PublishOutcome, model::Plan},
    crate::publication::{
        RDF_TYPE,
        diagnostics::{Diagnostic, FailureMode, Severity},
        term,
    },
    anyhow::Result,
    oxrdf::{Literal, NamedNode, Quad, Term},
    std::path::Path,
};

const GRAPH: &str = "urn:grapharch:publications";

pub(super) async fn begin(
    plan: &Plan,
    directory: &Path,
    job: &str,
    mode: FailureMode,
) -> Result<()> {
    let id = NamedNode::new(format!("urn:grapharch:publication:{job}"))?;
    let directory = url::Url::from_directory_path(directory).map_err(|_| {
        anyhow::anyhow!("publication directory is not an absolute file URL")
    })?;
    // bounded: fixed publication provenance, without copying source assertions.
    let fields: [(NamedNode, Term); 10] = [
        (
            NamedNode::new_unchecked(RDF_TYPE),
            term("PublicationJob").into(),
        ),
        (
            term("executionPolicy"),
            Literal::new_simple_literal(match mode {
                FailureMode::FailFast => "fail-fast",
                FailureMode::KeepGoing => "keep-going",
            })
            .into(),
        ),
        (term("project"), plan.project.clone().into()),
        (term("target"), plan.target.clone().into()),
        (
            term("inputRevision"),
            plan.revision.clone().into(),
        ),
        (term("plan"), plan.handle.id.clone().into()),
        (
            term("planSnapshot"),
            plan.handle.snapshot.clone().into(),
        ),
        (
            term("artifactDirectory"),
            NamedNode::new(directory.as_str())?.into(),
        ),
        (
            term("renderer"),
            Literal::new_simple_literal(&plan.renderer).into(),
        ),
        (
            term("status"),
            Literal::new_simple_literal("rendering").into(),
        ),
    ];
    plan.dataset
        .run(move |store| {
            store.transaction::<_, std::io::Error>(|mut transaction| {
                for (predicate, object) in &fields {
                    transaction.insert(
                        Quad::new(
                            id.clone(),
                            predicate.clone(),
                            object.clone(),
                            NamedNode::new_unchecked(GRAPH),
                        )
                        .as_ref(),
                    )?;
                }
                Ok(())
            })?;
            Ok(())
        })
        .await
}

pub(super) async fn finish(
    plan: &Plan,
    job: &str,
    result: &Result<PublishOutcome>,
) -> Result<()> {
    let id = NamedNode::new(format!("urn:grapharch:publication:{job}"))?;
    let status = Literal::new_simple_literal(match result {
        Ok(PublishOutcome::Published(publication))
            if !publication.manifest.diagnostics.is_empty() =>
        {
            "published-with-errors"
        },
        Ok(PublishOutcome::Published(_)) => "published",
        Ok(PublishOutcome::Superseded) => "superseded",
        Err(_) => "failed",
    });
    let diagnostics = match result {
        Ok(PublishOutcome::Published(publication)) => {
            publication.manifest.diagnostics.clone()
        },
        Err(error) => {
            vec![Diagnostic {
                severity:        Severity::Error,
                code:            "publication-failed".into(),
                message:         format!("{error:#}"),
                reference:       String::new(),
                source_fragment: None,
            }]
        },
        _ => Vec::new(),
    };
    let quads = super::diagnostic_graph::quads(plan, job, &diagnostics)?;
    plan.dataset
        .run(move |store| {
            let update = format!(
                "DELETE {{ GRAPH <{GRAPH}> {{ {id} {} ?old }} }} INSERT {{ \
                 GRAPH <{GRAPH}> {{ {id} {} {status} }} }} WHERE {{ GRAPH \
                 <{GRAPH}> {{ {id} {} ?old }} }}",
                term("status"),
                term("status"),
                term("status")
            );
            store.transaction::<_, std::io::Error>(|mut transaction| {
                transaction
                    .update(update.as_str())
                    .map_err(std::io::Error::other)?;
                for quad in &quads {
                    transaction.insert(quad.as_ref())?;
                }
                Ok(())
            })?;
            Ok(())
        })
        .await
}
