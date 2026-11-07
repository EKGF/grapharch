//! Compose retained native and authored fragments as navigable pages.
use {
    super::ComposedPage,
    crate::publication::{
        RDF_TYPE,
        dataset::{Dataset, Stage, identity},
        term,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::Result,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};
pub(super) fn page_id(processor: &NamedNode, subject: &NamedNode) -> NamedNode {
    identity("maturity-page", &[
        processor.as_str(),
        subject.as_str(),
    ])
}
pub(super) fn q(
    subject: &NamedNode,
    predicate: &str,
    value: impl Into<Term>,
) -> Quad {
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
pub(super) async fn write(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    processor: &NamedNode,
    pages: Vec<ComposedPage>,
) -> Result<()> {
    let snapshot = dataset.snapshot_for(project).await?;
    let view = crate::publication::view::RevisionView::new(
        dataset.clone(),
        snapshot.clone(),
    );
    let navigation =
        super::navigation::Placement::read(&view, processor).await?;
    let owner = identity("maturity-page-bindings", &[processor.as_str()]);
    let contribution = identity("contribution", &[
        project,
        owner.as_str(),
        processor.as_str(),
    ]);
    let mut stream = writer
        .begin_stage(
            project,
            owner.as_str(),
            processor.as_str(),
            &format!("maturity-pages-v1:{snapshot}"),
            Stage::Document,
        )
        .await?;
    for quad in [
        q(&owner, "type", term("Documentor")),
        q(&contribution, "type", term("Contribution")),
        q(&contribution, "project", NamedNode::new(project)?),
        q(&contribution, "processor", owner.clone()),
        q(
            &contribution,
            "source",
            Literal::new_simple_literal(processor.as_str()),
        ),
        q(
            &contribution,
            "revision",
            Literal::new_simple_literal(format!(
                "maturity-pages-v1:{snapshot}"
            )),
        ),
    ] {
        stream.write(quad).await?;
    }
    for composed in pages {
        let page = composed.page;
        if let Some(navigation) = &navigation {
            navigation.write(&mut stream, processor, &page).await?;
        }
        let id = page_id(processor, &page.subject);
        let header = identity("maturity-page-heading", &[id.as_str()]);
        for quad in [
            q(&id, "type", term("Page")),
            q(
                &id,
                "sourcePath",
                Literal::new_simple_literal(format!("{}index.md", page.route)),
            ),
            q(
                &id,
                "route",
                Literal::new_simple_literal(&page.route),
            ),
            q(&id, "navigationOrder", Literal::from(page.order)),
            q(&id, "navigationHidden", Literal::from(false)),
            q(&id, "type", term("Topic")),
        ] {
            stream.write(quad).await?;
        }
        stream
            .write(Quad::new(
                id.clone(),
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                Literal::new_simple_literal(&page.title),
                GraphName::DefaultGraph,
            ))
            .await?;
        if let Some(parent) = page.parent {
            stream
                .write(q(&id, "parentTopic", page_id(processor, &parent)))
                .await?;
        }
        generated(
            &mut stream,
            &header,
            &page.subject,
            &format!("# {}\n\n", page.title.replace('#', "\\#")),
            &snapshot,
            &contribution,
        )
        .await?;
        if let Some(body) = composed.generated_body {
            generated(
                &mut stream,
                &page.summary,
                &page.subject,
                &body,
                &snapshot,
                &contribution,
            )
            .await?;
        }
        part(&mut stream, &id, &header, 0).await?;
        part(&mut stream, &id, &page.summary, 1).await?;
        for (index, companion) in composed.companions.into_iter().enumerate() {
            stream
                .write(q(
                    &companion.fragment,
                    "sourcePath",
                    Literal::new_simple_literal(&companion.source_path),
                ))
                .await?;
            let heading = identity("maturity-section-heading", &[
                id.as_str(),
                companion.title,
            ]);
            stream
                .write(q(
                    &heading,
                    "routeAlias",
                    Literal::new_simple_literal(
                        crate::publication::collect::page::route(
                            &companion.source_path,
                            None,
                        )?,
                    ),
                ))
                .await?;
            generated(
                &mut stream,
                &heading,
                &heading,
                &format!("## {}\n\n", companion.title),
                &snapshot,
                &contribution,
            )
            .await?;
            part(&mut stream, &id, &heading, 2 + index as u32 * 2).await?;
            part(
                &mut stream,
                &id,
                &companion.fragment,
                3 + index as u32 * 2,
            )
            .await?;
        }
    }
    stream.complete().await?;
    Ok(())
}
async fn generated(
    stream: &mut ContributionStream,
    fragment: &NamedNode,
    subject: &NamedNode,
    body: &str,
    snapshot: &NamedNode,
    contribution: &NamedNode,
) -> Result<()> {
    for quad in [
        q(fragment, "type", term("Fragment")),
        q(fragment, "subject", subject.clone()),
        q(
            fragment,
            "format",
            Literal::new_simple_literal("text/markdown"),
        ),
        q(
            fragment,
            "body",
            Literal::new_simple_literal(body),
        ),
        q(fragment, "inputRevision", snapshot.clone()),
        q(fragment, "contribution", contribution.clone()),
    ] {
        stream.write(quad).await?;
    }
    Ok(())
}
async fn part(
    stream: &mut ContributionStream,
    page: &NamedNode,
    fragment: &NamedNode,
    order: u32,
) -> Result<()> {
    let topic = identity("maturity-page-topic", &[
        page.as_str(),
        fragment.as_str(),
    ]);
    for quad in [
        q(page, "topic", topic.clone()),
        q(&topic, "type", term("Topic")),
        q(&topic, "fragment", fragment.clone()),
        q(&topic, "partOf", page.clone()),
        q(fragment, "partOf", page.clone()),
        q(fragment, "order", Literal::from(order)),
    ] {
        stream.write(quad).await?;
    }
    Ok(())
}
