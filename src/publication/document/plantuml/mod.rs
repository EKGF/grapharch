//! Convert retained PlantUML source into validated SVG before publication
//! planning.
mod engine;
mod runner;
pub(crate) use engine::LocalRenderer;
use {
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
    std::{collections::BTreeSet, ffi::OsStr},
};
/// Document configured PlantUML sources using a locally installed offline
/// renderer. HTML/PDF renderers subsequently consume the retained SVG without
/// invoking it.
pub async fn document(
    dataset: &Dataset,
    project: &str,
    writer: &GraphWriter,
) -> Result<()> {
    document_with(dataset, project, writer, OsStr::new("plantuml")).await
}
async fn document_with(
    dataset: &Dataset,
    project: &str,
    writer: &GraphWriter,
    program: &OsStr,
) -> Result<()> {
    let renderer = LocalRenderer::new(program);
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_collected_for(project).await?,
    );
    let pattern = view.pattern(
        &format!(
            "?diagram a {} ; {} \"plantuml\"",
            term("Example"),
            term("language")
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?diagram WHERE {{ {pattern} }} LIMIT 33"
        ))
        .await?;
    let mut assets = BTreeSet::new(); // bounded: at most 32 configured source diagrams.
    let mut version = None;
    while let Some(row) = rows.next().await? {
        let Term::NamedNode(diagram) = &row["diagram"] else {
            anyhow::bail!("PlantUML diagram must be an IRI");
        };
        let asset = view.node(diagram, "asset").await?;
        ensure!(
            assets.len() < 32 && assets.insert(asset.clone()),
            "PlantUML source budget exceeded or outputs collide"
        );
        ensure!(
            view.text(diagram, "rendererExecutable").await? == "plantuml",
            "PlantUML only permits the explicitly configured local plantuml \
             tool"
        );
        let code = view.text(diagram, "body").await?;
        ensure!(
            code.len() <= 128 * 1024,
            "retained PlantUML source exceeds 128 KiB"
        );
        let version = match &version {
            Some(value) => value,
            None => version.insert(renderer.version().await?),
        };
        let output = view.text(diagram, "route").await?;
        let processor = identity("plantuml-documentor", &[project]);
        let revision = format!(
            "plantuml-svg-v1:{}:{}",
            version,
            hex::encode(Sha256::digest(code.as_bytes()))
        );
        if dataset
            .is_current(
                Stage::Document,
                project,
                processor.as_str(),
                &output,
                &revision,
            )
            .await?
        {
            continue;
        }
        let mut stream = writer
            .begin_stage(
                project,
                processor.as_str(),
                &output,
                &revision,
                Stage::Document,
            )
            .await?;
        let result = async {
            let svg = renderer.svg(&code).await?;
            ImageAsset {
                project,
                processor: processor.as_str(),
                source: &output,
                id: &asset,
            }
            .write(&svg, "image/svg+xml", &mut stream)
            .await?;
            stream
                .write(Quad::new(
                    asset.clone(),
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/provenance",
                    ),
                    Literal::new_simple_literal(version),
                    GraphName::DefaultGraph,
                ))
                .await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            stream.abort().await?;
            return Err(error);
        }
        stream.complete().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
