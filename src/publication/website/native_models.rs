//! Explicit model-aware publication composition; no legacy generator runtime.
#[cfg(test)]
mod acceptance;
mod bindings;
use {
    crate::publication::{
        collect::{Collector, MarkdownFile},
        dataset::{Dataset, identity},
        document::{
            Documentor,
            MaturityModelDocumentor,
            maturity::pages::Page,
        },
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path},
};
const SECTIONS: [(&str, &str); 7] = [
    ("background-and-intro", "Introduction"),
    ("dimensions", "Dimensions"),
    ("levels", "Levels"),
    ("value", "Value"),
    ("traditional-approach", "Traditional approach"),
    ("ekg-approach", "EKG approach"),
    ("use-cases", "Use cases"),
];
struct Companion {
    title:    &'static str,
    fragment: NamedNode,
}
struct ComposedPage {
    page:           Page,
    companions:     Vec<Companion>,
    generated_body: Option<String>,
}

pub(super) async fn collect(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    _concurrency: usize,
) -> Result<()> {
    let snapshot = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let mut definitions = view
        .objects(&NamedNode::new(project)?, "documentor")
        .await?;
    let mut count = 0;
    while let Some(row) = definitions.next().await? {
        count += 1;
        ensure!(
            count <= 8,
            "configured documentors exceed eight instances"
        );
        let Term::NamedNode(processor) = &row["value"] else {
            bail!("documentor must be an IRI")
        };
        ensure!(
            view.has_type(processor, "Documentor").await?,
            "configured documentor must be typed Documentor"
        );
        let kind = view.text(processor, "kind").await?;
        if kind == "maturity-overview" {
            let doc = MaturityModelDocumentor::new(
                dataset.clone(),
                snapshot.clone(),
                project,
            )?;
            let (page, body) = doc
                .overview_page(&view.text(processor, "route").await?)
                .await?;
            bindings::write(dataset, writer, project, processor, vec![
                ComposedPage {
                    page,
                    companions: Vec::new(),
                    generated_body: Some(body),
                },
            ])
            .await?;
            continue;
        }
        ensure!(
            kind == "maturity-model",
            "unsupported configured documentor kind: {kind}"
        );
        let source = view.text(processor, "source").await?;
        ensure!(
            !source.is_empty() &&
                Path::new(&source)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "maturity companion source must be relative without traversal"
        );
        let source = tokio::fs::canonicalize(root.join(source)).await?;
        ensure!(
            source.starts_with(root) &&
                tokio::fs::metadata(&source).await?.is_dir(),
            "maturity companions must stay within project source root"
        );
        let doc = MaturityModelDocumentor::new(
            dataset.clone(),
            snapshot.clone(),
            project,
        )?;
        let pages = doc
            .website_pages(&view.text(processor, "route").await?)
            .await?;
        let include_roots = super::discovery::include_roots(
            root,
            &view,
            &NamedNode::new(project)?,
        )
        .await?;
        let mut composed = Vec::new(); // bounded: native website limit is 256 pages, each with seven companions.
        for page in pages {
            let mut companions = Vec::new();
            if let Some(directory) = &page.companion_directory {
                for (name, title) in SECTIONS {
                    let path =
                        source.join(directory).join(format!("{name}.md"));
                    let canonical = match tokio::fs::canonicalize(&path).await {
                        Ok(value) => value,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            continue
                        },
                        Err(error) => return Err(error.into()),
                    };
                    ensure!(
                        canonical.starts_with(&source),
                        "maturity companion escapes source root"
                    );
                    let owner = identity("maturity-companion-collector", &[
                        processor.as_str(),
                        page.subject.as_str(),
                        name,
                    ]);
                    let collector =
                        MarkdownFile::new(project, owner.as_str(), canonical)?
                            .with_include_roots(include_roots.clone())?;
                    companions.push(Companion {
                        title,
                        fragment: collector.fragment_id(),
                    });
                    collector.collect(writer).await?;
                }
            }
            composed.push(ComposedPage {
                page,
                companions,
                generated_body: None,
            });
        }
        doc.document(writer).await?;
        bindings::write(dataset, writer, project, processor, composed).await?;
    }
    Ok(())
}
