//! Actual EKGF use-case instances become fragments and retained vector
//! diagrams.
mod model;
mod svg;
use {
    super::{Documentor, annotated_term::escape},
    crate::publication::{
        RDF_TYPE,
        collect::ImageAsset,
        dataset::{Dataset, Stage, identity},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    model::Tree,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};
/// Document explicit EKGF UseCase instances from a pinned collected revision.
pub struct UseCaseDocumentor {
    dataset:   Dataset,
    snapshot:  NamedNode,
    project:   NamedNode,
    processor: NamedNode,
}
impl UseCaseDocumentor {
    /// Bind one documentor to a project and immutable collected snapshot.
    pub fn new(
        dataset: Dataset,
        snapshot: NamedNode,
        project: &str,
    ) -> Result<Self> {
        Ok(Self {
            dataset,
            snapshot,
            project: NamedNode::new(project)?,
            processor: identity("use-case-documentor", &[project]),
        })
    }

    /// Stable identity of a generated use-case fragment.
    pub fn fragment_id(&self, subject: &NamedNode) -> NamedNode {
        identity("use-case-fragment", &[
            self.project.as_str(),
            subject.as_str(),
        ])
    }

    /// Source-owned hierarchy order, with independent roots and sorted
    /// children.
    pub async fn subjects(&self) -> Result<Vec<NamedNode>> {
        Ok(Tree::read(
            &self.dataset,
            &RevisionView::new(self.dataset.clone(), self.snapshot.clone()),
        )
        .await?
        .order())
    }

    async fn retire(&self, tree: &Tree, writer: &GraphWriter) -> Result<()> {
        let query = format!(
            "SELECT DISTINCT ?source WHERE {{ GRAPH <{}> {{ ?owner {} \
             ?version . ?version {} ?attempt . ?attempt {} {} ; {} {} ; {} \
             ?source }} }}",
            crate::publication::dataset::CONTROL,
            term("head"),
            term("data"),
            term("project"),
            self.project,
            term("collector"),
            self.processor,
            term("source")
        );
        let mut rows = self.dataset.select(query).await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(source) = &row["source"] else {
                anyhow::bail!("invalid use-case documentor source")
            };
            if !tree.nodes.contains_key(&NamedNode::new(source.value())?) {
                writer
                    .remove(
                        self.project.as_str(),
                        self.processor.as_str(),
                        source.value(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
fn q(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
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
fn link(tree: &Tree, id: &NamedNode) -> String {
    format!(
        "[{}]({})",
        escape(&tree.nodes[id].label),
        id.as_str()
    )
}
#[async_trait]
impl Documentor for UseCaseDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let view =
            RevisionView::new(self.dataset.clone(), self.snapshot.clone());
        let tree = Tree::read(&self.dataset, &view).await?;
        for id in tree.order() {
            let node = &tree.nodes[&id];
            let description =
                match super::description::read(&view, &id, "en").await? {
                    Some(value) => super::description::nested(&value, 3)?,
                    None => {
                        escape(
                            &model::text(
                                &view,
                                &id,
                                "http://www.w3.org/2000/01/rdf-schema#comment",
                                true,
                            )
                            .await?,
                        )
                    },
                };
            let related_resources =
                model::related(&self.dataset, &view, &id).await?;
            let fragment = self.fragment_id(&id);
            let asset = identity("use-case-diagram", &[
                self.project.as_str(),
                id.as_str(),
            ]);
            let diagram = svg::diagram(&tree, &id)?;
            let mut body = format!(
                "{}\n\n![Use-case neighborhood for {}]({})\n\nSolid lines \
                 show ownership; dashed lines show reuse. The current use \
                 case is boxed.\n\n**Identifier:** `{}`\n\n**Ownership \
                 parent:** ",
                description,
                escape(&node.label),
                asset.as_str(),
                id.as_str()
            );
            let mut references = std::collections::BTreeSet::new(); // bounded: at most the validated tree's 4096 subjects.
            if let Some(parent) = &node.parent {
                body.push_str(&link(&tree, parent));
                references.insert(parent.clone());
            } else {
                body.push_str("none asserted (an independent root)");
            }
            body.push_str("\n\n**Reused in:** ");
            if node.used_in.is_empty() {
                body.push_str("none asserted");
            }
            for (i, target) in node.used_in.iter().enumerate() {
                if i > 0 {
                    body.push_str(", ");
                }
                body.push_str(&link(&tree, target));
                references.insert(target.clone());
            }
            for (heading, children) in [
                ("Ownership children", tree.children(&id, false)),
                (
                    "Reused capabilities",
                    tree.children(&id, true)
                        .into_iter()
                        .filter(|child| tree.nodes[child].used_in.contains(&id))
                        .collect(),
                ),
            ] {
                if !children.is_empty() {
                    body.push_str(&format!("\n\n### {heading}\n\n"));
                    for child in children {
                        body.push_str(&format!("- {}\n", link(&tree, &child)));
                        references.insert(child);
                    }
                }
            }
            if !related_resources.is_empty() {
                body.push_str(
                    "\n\n### Related resources\n\n| Relationship | Resource \
                     and source description |\n| --- | --- |\n",
                );
                for related in &related_resources {
                    body.push_str(&format!(
                        "| `ekg-use-case:{}` | **{}** — {} (`{}`) |\n",
                        related.predicate,
                        escape(&related.label),
                        escape(&related.description),
                        related.subject.as_str()
                    ));
                }
            }
            ensure!(
                body.len() <= 256 * 1024,
                "use-case fragment exceeds byte budget"
            );
            let revision = format!(
                "use-case-v1:{}",
                hex::encode(Sha256::digest(
                    format!("{body}{diagram}").as_bytes()
                ))
            );
            if self
                .dataset
                .is_current(
                    Stage::Document,
                    self.project.as_str(),
                    self.processor.as_str(),
                    id.as_str(),
                    &revision,
                )
                .await?
            {
                continue;
            }
            let contribution = identity("contribution", &[
                self.project.as_str(),
                self.processor.as_str(),
                id.as_str(),
            ]);
            let mut stream = writer
                .begin_stage(
                    self.project.as_str(),
                    self.processor.as_str(),
                    id.as_str(),
                    &revision,
                    Stage::Document,
                )
                .await?;
            ImageAsset {
                project:   self.project.as_str(),
                processor: self.processor.as_str(),
                source:    id.as_str(),
                id:        &asset,
            }
            .write(diagram.as_bytes(), "image/svg+xml", &mut stream)
            .await?;
            let reference =
                identity("use-case-diagram-reference", &[fragment.as_str()]);
            for quad in [
                q(&self.processor, "type", term("Documentor")),
                q(&contribution, "type", term("Contribution")),
                q(&contribution, "project", self.project.clone()),
                q(&contribution, "processor", self.processor.clone()),
                q(
                    &contribution,
                    "source",
                    Literal::new_simple_literal(id.as_str()),
                ),
                q(
                    &contribution,
                    "revision",
                    Literal::new_simple_literal(revision),
                ),
                q(&fragment, "type", term("Fragment")),
                q(&fragment, "subject", id.clone()),
                q(
                    &fragment,
                    "format",
                    Literal::new_simple_literal("text/markdown"),
                ),
                q(
                    &fragment,
                    "body",
                    Literal::new_simple_literal(body),
                ),
                q(&fragment, "contribution", contribution),
                q(&fragment, "inputRevision", self.snapshot.clone()),
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
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/identifier",
                    ),
                    Literal::new_simple_literal(asset.as_str()),
                    GraphName::DefaultGraph,
                ),
            ] {
                stream.write(quad).await?;
            }
            for target in references {
                stream.write(q(&fragment, "reference", target)).await?;
            }
            stream.complete().await?;
        }
        self.retire(&tree, writer).await
    }
}
