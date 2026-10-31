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
            if publication
                .manifest
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, Severity::Error)) =>
        {
            "published-with-errors"
        },
        Ok(PublishOutcome::Published(publication))
            if !publication.manifest.diagnostics.is_empty() =>
        {
            "published-with-warnings"
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
    let mut quads = super::diagnostic_graph::quads(plan, job, &diagnostics)?;
    if let Ok(PublishOutcome::Published(publication)) = result {
        for book in &publication.manifest.books {
            let operation =
                crate::publication::dataset::identity("book-publication", &[
                    job, &book.plan,
                ]);
            let fields: [(NamedNode, Term); 9] = [
                (
                    NamedNode::new_unchecked(RDF_TYPE),
                    term("BookPublication").into(),
                ),
                (
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/isPartOf",
                    ),
                    id.clone().into(),
                ),
                (
                    NamedNode::new_unchecked(
                        "http://www.w3.org/ns/prov#wasDerivedFrom",
                    ),
                    NamedNode::new(&book.plan)?.into(),
                ),
                (
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/subject",
                    ),
                    NamedNode::new(&book.subject)?.into(),
                ),
                (
                    term("inputRevision"),
                    plan.revision.clone().into(),
                ),
                (
                    term("status"),
                    Literal::new_simple_literal(
                        if book.status == "ready" {
                            "published"
                        } else {
                            "published-with-errors"
                        },
                    )
                    .into(),
                ),
                (term("reused"), Literal::from(book.reused).into()),
                (term("project"), plan.project.clone().into()),
                (
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/identifier",
                    ),
                    Literal::new_simple_literal(&book.digest).into(),
                ),
            ];
            for (predicate, object) in fields {
                quads.push(Quad::new(
                    operation.clone(),
                    predicate,
                    object,
                    NamedNode::new_unchecked(GRAPH),
                ));
            }
            for (route, digest) in [
                (Some(book.html.as_str()), None),
                (book.pdf.as_deref(), book.pdf_digest.as_deref()),
            ] {
                if let Some(route) = route {
                    let artifact = crate::publication::dataset::identity(
                        "book-artifact",
                        &[job, &book.plan, route],
                    );
                    let location = url::Url::from_file_path(
                        publication.directory.join(route),
                    )
                    .map_err(|_| {
                        anyhow::anyhow!("invalid book artifact location")
                    })?;
                    for (s, p, o) in [
                        (
                            operation.clone(),
                            NamedNode::new_unchecked(
                                "http://www.w3.org/ns/prov#generated",
                            ),
                            artifact.clone(),
                        ),
                        (
                            artifact.clone(),
                            NamedNode::new_unchecked(RDF_TYPE),
                            NamedNode::new_unchecked(
                                "http://www.w3.org/ns/prov#Entity",
                            ),
                        ),
                        (
                            artifact.clone(),
                            NamedNode::new_unchecked(
                                "http://www.w3.org/ns/prov#atLocation",
                            ),
                            NamedNode::new(location.as_str())?,
                        ),
                    ] {
                        quads.push(Quad::new(
                            s,
                            p,
                            o,
                            NamedNode::new_unchecked(GRAPH),
                        ));
                    }
                    if let Some(digest) = digest {
                        quads.push(Quad::new(
                            artifact,
                            NamedNode::new_unchecked(
                                "http://purl.org/dc/terms/identifier",
                            ),
                            Literal::new_simple_literal(format!(
                                "sha256:{digest}"
                            )),
                            NamedNode::new_unchecked(GRAPH),
                        ));
                    }
                }
            }
        }
    }
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
