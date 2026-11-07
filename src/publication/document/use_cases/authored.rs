//! Attach native topology to existing authored pages without reproducing prose.
use {
    super::{
        escape,
        link,
        model::{Tree, UC},
        q,
        svg,
    },
    crate::publication::{
        collect::ImageAsset,
        dataset::{Dataset, Stage, identity},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};
/// Generate relationships only for explicitly collected source-owned use cases.
pub(crate) async fn document(
    dataset: &Dataset,
    project: &str,
    writer: &GraphWriter,
) -> Result<()> {
    let snapshot = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let typed = view.pattern(
        &format!(
            "?subject a <{UC}UseCase> ; {} ?path",
            term("sourcePath")
        ),
        0,
    );
    let mut sources = dataset
        .select(format!(
            "SELECT DISTINCT ?subject ?path WHERE {{ {typed} }} ORDER BY \
             ?subject LIMIT 257"
        ))
        .await?;
    let mut tree = None;
    let processor = identity("authored-use-case-documentor", &[project]);
    let mut count = 0;
    while let Some(row) = sources.next().await? {
        count += 1;
        ensure!(
            count <= 256,
            "authored use-case documentation exceeds 256 pages"
        );
        let (Term::NamedNode(subject), Term::Literal(path)) =
            (&row["subject"], &row["path"])
        else {
            anyhow::bail!("invalid authored use-case source binding")
        };
        if tree.is_none() {
            tree = Some(Tree::read(dataset, &view).await?);
        }
        let tree = tree.as_ref().unwrap();
        let pages = view.pattern(
            &format!(
                "?page a {} ; {} {}",
                term("Page"),
                term("sourcePath"),
                path
            ),
            0,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?page WHERE {{ {pages} }} LIMIT 2"
            ))
            .await?;
        let page = rows.next().await?.ok_or_else(|| {
            anyhow::anyhow!(
                "authored use case has no collected page: {}",
                path.value()
            )
        })?;
        let Term::NamedNode(page) = &page["page"] else {
            anyhow::bail!("use-case page must be named")
        };
        ensure!(
            rows.next().await?.is_none(),
            "ambiguous authored use-case page: {}",
            path.value()
        );
        let node = &tree.nodes[subject];
        let fragment = identity("authored-use-case-topology", &[
            project,
            subject.as_str(),
        ]);
        let topic = identity("authored-use-case-topic", &[
            project,
            subject.as_str(),
        ]);
        let asset = identity("authored-use-case-diagram", &[
            project,
            subject.as_str(),
        ]);
        let summary = crate::publication::document::description::read(
            &view, subject, "en",
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("authored use-case summary missing: {subject}")
        })?;
        let mut body = format!(
            "## Use-case relationships\n\n![Use-case neighborhood for \
             {}]({asset})\n\n{summary}\n\nSolid lines show ownership; dashed \
             lines show reuse. The current use case is boxed.\n\n**Ownership \
             parent:** ",
            escape(&node.label)
        );
        if let Some(parent) = &node.parent {
            body.push_str(&link(tree, parent));
        } else {
            body.push_str("Independent root");
        }
        body.push_str("\n\n**Used in:** ");
        if node.used_in.is_empty() {
            body.push_str("None asserted");
        } else {
            for (index, target) in node.used_in.iter().enumerate() {
                if index > 0 {
                    body.push_str(", ");
                }
                body.push_str(&link(tree, target));
            }
        }
        for (heading, reuse) in
            [("Ownership children", false), ("Reused components", true)]
        {
            let children = if reuse {
                tree.sorted(
                    tree.nodes
                        .iter()
                        .filter(|(_, node)| node.used_in.contains(subject))
                        .map(|(id, _)| id.clone())
                        .collect(),
                )
            } else {
                tree.children(subject, false)
            };
            if !children.is_empty() {
                body.push_str(&format!("\n\n**{heading}:**\n\n"));
                for child in children {
                    body.push_str(&format!("- {}\n", link(tree, &child)));
                }
            }
        }
        ensure!(
            body.len() <= 256 * 1024,
            "authored topology fragment exceeds 256 KiB"
        );
        let diagram = svg::diagram(tree, subject)?;
        let revision = format!(
            "authored-topology-v1:{}",
            hex::encode(Sha256::digest(
                format!("{body}{diagram}").as_bytes()
            ))
        );
        let mut stream = writer
            .begin_stage(
                project,
                processor.as_str(),
                subject.as_str(),
                &revision,
                Stage::Document,
            )
            .await?;
        ImageAsset {
            project,
            processor: processor.as_str(),
            source: subject.as_str(),
            id: &asset,
        }
        .write(diagram.as_bytes(), "image/svg+xml", &mut stream)
        .await?;
        let reference = identity("authored-use-case-image-reference", &[
            fragment.as_str(),
        ]);
        let contribution = identity("contribution", &[
            project,
            processor.as_str(),
            subject.as_str(),
        ]);
        for quad in [
            q(&processor, "type", term("Documentor")),
            q(&contribution, "type", term("Contribution")),
            q(&contribution, "project", NamedNode::new(project)?),
            q(&contribution, "processor", processor.clone()),
            q(
                &contribution,
                "source",
                Literal::new_simple_literal(subject.as_str()),
            ),
            q(
                &contribution,
                "revision",
                Literal::new_simple_literal(&revision),
            ),
            q(&fragment, "contribution", contribution),
            q(page, "topic", topic.clone()),
            q(&topic, "type", term("Topic")),
            q(&topic, "type", term("Component")),
            q(&topic, "partOf", page.clone()),
            q(&topic, "fragment", fragment.clone()),
            q(
                &topic,
                "headingAnchor",
                Literal::new_simple_literal("use-case-relationships"),
            ),
            q(&fragment, "type", term("Fragment")),
            q(&fragment, "subject", subject.clone()),
            q(&fragment, "partOf", page.clone()),
            q(
                &fragment,
                "body",
                Literal::new_simple_literal(body),
            ),
            q(
                &fragment,
                "format",
                Literal::new_simple_literal("text/markdown"),
            ),
            q(&fragment, "order", Literal::from(10000_u32)),
            q(&fragment, "inputRevision", snapshot.clone()),
            q(&fragment, "asset", asset.clone()),
            q(&fragment, "reference", reference.clone()),
            q(&reference, "type", term("ExternalReference")),
            q(&reference, "asset", asset.clone()),
            q(
                &reference,
                "url",
                Literal::new_simple_literal(asset.as_str()),
            ),
            Quad::new(
                reference,
                NamedNode::new_unchecked("http://purl.org/dc/terms/identifier"),
                Literal::new_simple_literal(asset.as_str()),
                GraphName::DefaultGraph,
            ),
        ] {
            stream.write(quad).await?;
        }
        stream.complete().await?;
    }
    Ok(())
}
