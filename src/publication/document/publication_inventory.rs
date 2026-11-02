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
    let mut body = format!(
        "---\nnav_order: {}\n---\n# {}\n\nSelect a published specification or \
         a current preview.\n\nInventory captured at {}. Only verified \
         open-review branches are advertised; existing unlisted preview URLs \
         remain routable.\n\n## Available versions\n\n",
        job.order,
        markdown_text(&job.title),
        markdown_text(&inventory.observed_at)
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
            version.url
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
    stream.complete().await?;
    Ok(())
}
