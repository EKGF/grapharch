//! Explicit authored use-case facts, one independently owned source per page.
mod paths;
use {
    super::{Collector, SourceIdentity, source::FileInput},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    serde::Deserialize,
    std::{collections::BTreeMap, path::Path, sync::Arc},
};
const UC: &str = "https://ekgf.org/ontology/use-case#";
#[derive(Deserialize)]
struct Metadata {
    title:   String,
    summary: String,
    #[serde(rename = "is-part-of")]
    parent:  Option<Vec<String>>,
    #[serde(default, rename = "is-used-in")]
    usage:   Vec<String>,
}
struct AuthoredUseCase {
    input: FileInput,
    id:    String,
    path:  String,
    nodes: Arc<BTreeMap<String, NamedNode>>,
}
/// Expand one explicit subtree into small, concurrent per-file collectors.
pub(crate) async fn configured(
    project: &str,
    processor: &NamedNode,
    source: &str,
    root: &Path,
    view: &RevisionView,
) -> Result<Vec<Arc<dyn Collector>>> {
    let content = crate::publication::website::discovery::content_root(
        root,
        view,
        &NamedNode::new(project)?,
    )
    .await?;
    let paths = paths::discover(root, source).await?;
    let mut nodes = BTreeMap::new(); // bounded: 256 explicitly discovered Markdown sources.
    for (id, _) in &paths {
        ensure!(
            nodes
                .insert(
                    id.clone(),
                    identity("authored-use-case", &[
                        project,
                        processor.as_str(),
                        id
                    ])
                )
                .is_none(),
            "duplicate authored use-case identity: {id}"
        );
    }
    let nodes = Arc::new(nodes);
    let mut work = Vec::new(); // bounded: one collector per discovered source, at most 256.
    for (id, path) in paths {
        let relative = super::page::source_path(&content, &path)?;
        let owner = identity("authored-use-case-collector", &[
            processor.as_str(),
            &id,
        ]);
        work.push(Arc::new(AuthoredUseCase {
            input: FileInput::new(project, owner.as_str(), path)?,
            id,
            path: relative,
            nodes: nodes.clone(),
        }) as Arc<dyn Collector>);
    }
    Ok(work)
}
#[async_trait]
impl Collector for AuthoredUseCase {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.input.project.as_str(),
            processor: self.input.collector.as_str(),
            source:    self.input.source()?,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let (source, revision) = self.input.authored().await?;
        let result = async {
            let normalized = source.replace("\r\n", "\n");
            let rest = normalized
                .strip_prefix("---\n")
                .context("authored use case requires frontmatter")?;
            let (frontmatter, _) = rest
                .split_once("\n---\n")
                .context("unterminated use-case frontmatter")?;
            let metadata: Metadata = serde_yaml::from_str(frontmatter)?;
            ensure!(
                !metadata.title.trim().is_empty() &&
                    metadata.title.len() <= 160,
                "use-case title must contain 1–160 bytes"
            );
            ensure!(
                !metadata.summary.trim().is_empty() &&
                    metadata.summary.len() <= 64 * 1024,
                "use-case summary must contain 1–65536 bytes"
            );
            let parent = metadata.parent.unwrap_or_else(|| {
                if self.id.contains('/') {
                    vec!["..".into()]
                } else {
                    Vec::new()
                }
            });
            ensure!(
                parent.len() <= 1 && metadata.usage.len() <= 64,
                "use case permits one ownership parent and 64 reuse \
                 relationships"
            );
            let subject = &self.nodes[&self.id];
            for quad in [
                Quad::new(
                    subject.clone(),
                    NamedNode::new_unchecked(RDF_TYPE),
                    NamedNode::new_unchecked(format!("{UC}UseCase")),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    subject.clone(),
                    NamedNode::new_unchecked(
                        "http://www.w3.org/2000/01/rdf-schema#label",
                    ),
                    Literal::new_simple_literal(&metadata.title),
                    GraphName::DefaultGraph,
                ),
                Quad::new(
                    subject.clone(),
                    NamedNode::new_unchecked(
                        crate::publication::document::description::MARKDOWN,
                    ),
                    Literal::new_simple_literal(&metadata.summary),
                    GraphName::DefaultGraph,
                ),
                self.input.quad(
                    subject,
                    "sourcePath",
                    Literal::new_simple_literal(&self.path),
                ),
            ] {
                stream.write(quad).await?;
            }
            for (predicate, references) in
                [("isPartOf", parent), ("isUsedIn", metadata.usage)]
            {
                for reference in references {
                    let id = paths::resolve(&self.id, &reference)?;
                    let target = self.nodes.get(&id).with_context(|| {
                        format!("unresolved {predicate} reference: {reference}")
                    })?;
                    ensure!(
                        target != subject,
                        "self-referential {predicate}: {reference}"
                    );
                    stream
                        .write(Quad::new(
                            subject.clone(),
                            NamedNode::new_unchecked(format!(
                                "{UC}{predicate}"
                            )),
                            target.clone(),
                            GraphName::DefaultGraph,
                        ))
                        .await?;
                }
            }
            Ok(format!("authored-use-case-v1:{revision}"))
        }
        .await;
        result.with_context(|| format!("authored use-case {}", self.path))
    }
}
