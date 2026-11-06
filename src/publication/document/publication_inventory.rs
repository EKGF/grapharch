//! Publication version pages are generated from retained inventory assertions.
use {
    crate::publication::{
        collect::{Collector, GeneratedPage, MarkdownFile},
        dataset::{Dataset, Stage},
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::Result,
    oxrdf::NamedNode,
};

pub(crate) mod model;
use model::{Inventory, configured_job};

fn markdown_text(text: &str) -> String {
    let mut output = String::new();
    for c in text.chars() {
        if "\\`*_{}[]<>#|!".contains(c) {
            output.push('\\');
        }
        output.push(c);
    }
    output
}

pub(crate) async fn document(
    dataset: &Dataset,
    project: &str,
    writer: &GraphWriter,
) -> Result<()> {
    let snapshot = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let Some(job) =
        configured_job(dataset, &view, &NamedNode::new(project)?).await?
    else {
        return Ok(())
    };
    let inventory = Inventory::read(dataset, &view, &job.subject).await?;
    let observed =
        chrono::DateTime::parse_from_rfc3339(&inventory.observed_at)?
            .with_timezone(&chrono::Utc)
            .format("%-d %B %Y at %H:%M UTC")
            .to_string();
    let mut body = format!(
        "---\nnav_order: {}\n---\n# {}\n\nSelect a published specification or \
         a current preview.\n\n## Available versions\n\nLast checked {}.\n\n",
        job.order,
        markdown_text(&job.title),
        observed
    );
    for version in inventory
        .versions
        .iter()
        .filter(|version| version.advertised)
    {
        body.push_str(&format!(
            "### {}\n\nBranch: {}\n\n[Open specification](<{}>)\n\n",
            markdown_text(&version.label),
            markdown_text(&version.branch),
            version.route.as_deref().unwrap_or(&version.url)
        ));
    }
    let revision = format!("publication-inventory-v1:{snapshot}");
    let page =
        MarkdownFile::generated(project, job.id.as_str(), GeneratedPage {
            source: job.subject.as_str(),
            id: &job.page,
            path: &job.path,
            title: &job.title,
            route: &job.route,
            body,
            revision: revision.clone(),
        })?;
    let mut stream = writer
        .begin_stage(
            project,
            job.id.as_str(),
            job.subject.as_str(),
            &revision,
            Stage::Document,
        )
        .await?;
    if let Err(error) = page.produce(&mut stream).await {
        stream.abort().await?;
        return Err(error);
    }
    // Retain explicitly assembled sibling routes as graph-owned link targets.
    // They are not Markdown pages in this website target's own scope.
    for version in &inventory.versions {
        if let Some(route) = &version.route {
            let reference =
                crate::publication::dataset::identity("publication-route", &[
                    job.page.as_str(),
                    route,
                ]);
            for (subject, predicate, object) in [
                (
                    job.page.clone(),
                    crate::publication::term("reference"),
                    oxrdf::Term::NamedNode(reference.clone()),
                ),
                (
                    reference.clone(),
                    NamedNode::new("http://purl.org/dc/terms/identifier")?,
                    oxrdf::Literal::new_simple_literal(route).into(),
                ),
                (
                    reference,
                    crate::publication::term("url"),
                    oxrdf::Literal::new_simple_literal(route).into(),
                ),
            ] {
                stream
                    .write(oxrdf::Quad::new(
                        subject,
                        predicate,
                        object,
                        oxrdf::GraphName::DefaultGraph,
                    ))
                    .await?;
            }
        }
    }
    stream.complete().await?;
    Ok(())
}
