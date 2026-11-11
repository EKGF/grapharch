//! Configured collector uses over one pinned Git content tree. Uses are read
//! from a definitions-only view, so collected content cannot register further
//! collectors or change their settings.
use {
    super::{
        Collector,
        HtmlPreambleFile,
        collect_all,
        git::{GitCodeFile, GitMarkdownFile, RepositoryScan},
        kind::CollectorKind,
    },
    crate::publication::{
        document::scope_inclusion,
        limits::AUTHORED_ASSETS,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::{collections::HashSet, path::Path, sync::Arc},
};

/// Maximum collector uses one project may configure over a Git tree.
pub const COLLECTOR_USES: usize = 64;

/// Build the configured existing collectors over `tree` and run them with
/// bounded concurrency. `definitions` must be a definitions-only view.
pub async fn collect(
    tree: &RepositoryScan,
    definitions: &RevisionView,
    project: &NamedNode,
    writer: &GraphWriter,
    concurrency: usize,
) -> Result<()> {
    let uses = Uses::read(definitions, project).await?;
    let mut work = Vec::<Arc<dyn Collector>>::new(); // bounded: 64 uses and 256 images.
    let mut fragments = HashSet::new(); // bounded: one fragment per use.
    let mut images = HashSet::new(); // bounded: AUTHORED_ASSETS sources.
    for processor in &uses.0 {
        let configured = Use { view: definitions, processor };
        work.extend(
            configured
                .collectors(tree, project, &mut fragments, &mut images)
                .await
                .with_context(|| {
                    format!("configured collector use {processor}")
                })?,
        );
    }
    collect_all(work, writer, concurrency).await
}

/// The project's collector uses in IRI order, independent of source order.
struct Uses(Vec<NamedNode>);

impl Uses {
    async fn read(view: &RevisionView, project: &NamedNode) -> Result<Self> {
        let mut rows = view.objects(project, "collector").await?;
        let mut uses = Vec::new();
        while let Some(row) = rows.next().await? {
            ensure!(
                uses.len() < COLLECTOR_USES,
                "{project} exceeds the {COLLECTOR_USES}-collector budget"
            );
            let Term::NamedNode(processor) = &row["value"] else {
                bail!("{project} ga:collector must identify an IRI")
            };
            uses.push(processor.clone());
        }
        uses.sort();
        Ok(Self(uses))
    }
}

struct Use<'a> {
    view:      &'a RevisionView,
    processor: &'a NamedNode,
}

impl Use<'_> {
    async fn collectors(
        &self,
        tree: &RepositoryScan,
        project: &NamedNode,
        fragments: &mut HashSet<NamedNode>,
        images: &mut HashSet<String>,
    ) -> Result<Vec<Arc<dyn Collector>>> {
        ensure!(
            self.view.has_type(self.processor, "CollectorUse").await?,
            "collector must be typed ga:CollectorUse"
        );
        let kind = CollectorKind::from_node(
            &self.view.node(self.processor, "collectorType").await?,
        )?;
        let blob = tree
            .blob(Path::new(
                &self.view.text(self.processor, "sourcePath").await?,
            ))
            .await?;
        let processor = self.processor.as_str();
        Ok(match kind {
            CollectorKind::Image => {
                vec![blob.image_collector(
                    processor,
                    self.view.node(self.processor, "asset").await?,
                )?]
            },
            CollectorKind::RdfFile => vec![blob.rdf_collector(processor)?],
            CollectorKind::CodeFile => {
                vec![Arc::new(GitCodeFile::new(
                    project.as_str(),
                    processor,
                    blob,
                    self.fragment(fragments).await?,
                    self.view.node(self.processor, "subject").await?,
                    &self.view.text(self.processor, "format").await?,
                )?)]
            },
            CollectorKind::HtmlPreamble => {
                vec![Arc::new(HtmlPreambleFile::from_git(
                    project.as_str(),
                    processor,
                    &blob,
                    self.fragment(fragments).await?,
                )?)]
            },
            CollectorKind::MarkdownFile | CollectorKind::MarkdownPage => {
                let page = kind == CollectorKind::MarkdownPage;
                let mut markdown = GitMarkdownFile::new(
                    blob,
                    processor,
                    self.fragment(fragments).await?,
                    page,
                )?;
                if page {
                    markdown = markdown.with_page_hints(
                        self.page_title().await?,
                        self.view.text(self.processor, "route").await?,
                    )?;
                }
                let mut work = Vec::new(); // bounded: 64 images per document.
                for image in markdown.image_collectors().await? {
                    if images.insert(image.identity()?.source.to_owned()) {
                        ensure!(
                            images.len() <= AUTHORED_ASSETS,
                            "Git content exceeds the {AUTHORED_ASSETS}-image \
                             budget"
                        );
                        work.push(image);
                    }
                }
                work.push(Arc::new(markdown));
                work
            },
            _ => {
                bail!(
                    "collectorType {kind:?} is not supported over a pinned \
                     Git tree"
                )
            },
        })
    }

    async fn fragment(
        &self,
        fragments: &mut HashSet<NamedNode>,
    ) -> Result<NamedNode> {
        let fragment = self.view.node(self.processor, "fragment").await?;
        scope_inclusion::fragment_scopes(self.view, self.processor, &fragment)
            .await?;
        ensure!(
            fragments.insert(fragment.clone()),
            "duplicate authored fragment {fragment}"
        );
        Ok(fragment)
    }

    async fn page_title(&self) -> Result<String> {
        match self
            .view
            .optional_by(
                self.processor,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            )
            .await?
        {
            Some(Term::Literal(title)) => Ok(title.value().to_owned()),
            Some(_) => bail!("page dct:title must be a literal"),
            None => bail!("page collector requires dct:title"),
        }
    }
}

#[cfg(test)]
mod tests;
