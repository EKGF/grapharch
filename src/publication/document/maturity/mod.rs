//! Native EKGF maturity-model fragments from a pinned collected revision.
mod model;
mod write;
use {
    super::{Documentor, annotated_term::escape},
    crate::publication::{
        dataset::{Dataset, identity},
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    model::{Kind, Model},
    oxrdf::NamedNode,
};
/// Generate maturity-model, pillar, capability-area, capability and level
/// fragments. Authored level assessment prose remains authored content, not
/// invented RDF facts.
pub struct MaturityModelDocumentor {
    dataset:   Dataset,
    snapshot:  NamedNode,
    project:   NamedNode,
    processor: NamedNode,
}
impl MaturityModelDocumentor {
    /// Bind to one project and immutable collected snapshot.
    pub fn new(
        dataset: Dataset,
        snapshot: NamedNode,
        project: &str,
    ) -> Result<Self> {
        Ok(Self {
            dataset,
            snapshot,
            project: NamedNode::new(project)?,
            processor: identity("maturity-documentor", &[project]),
        })
    }

    /// Stable generated fragment identity, independent of publication
    /// placement.
    pub fn fragment_id(&self, subject: &NamedNode) -> NamedNode {
        identity("maturity-fragment", &[
            self.project.as_str(),
            subject.as_str(),
        ])
    }

    /// Source hierarchy order: models, pillars, areas, capabilities and levels.
    pub async fn subjects(&self) -> Result<Vec<NamedNode>> {
        Ok(Model::read(&self.dataset, &self.view()).await?.order())
    }

    fn view(&self) -> RevisionView {
        RevisionView::new(self.dataset.clone(), self.snapshot.clone())
    }

    async fn body(&self, model: &Model, id: &NamedNode) -> Result<String> {
        let node = &model.nodes[id];
        let view = self.view();
        let mut body = format!(
            "**{}:** {}\n\n",
            node.kind.label(),
            escape(&node.label)
        );
        if !node.number.is_empty() {
            body.push_str(&format!(
                "**Capability number:** {}\n\n",
                escape(&node.number)
            ));
        }
        let comment = model::text(&view, id, model::COMMENT, false).await?;
        if !comment.is_empty() {
            body.push_str(&format!("{}\n\n", escape(&comment)));
        }
        let description =
            model::text(&view, id, model::DESCRIPTION, false).await?;
        if !description.is_empty() {
            body.push_str(&super::description::nested(
                &dedent(&description),
                3,
            )?);
            body.push_str("\n\n");
        }
        if let Some(parent) = &node.parent {
            body.push_str(&format!(
                "**{}:** {}\n\n",
                model.nodes[parent].kind.label(),
                link(model, parent)
            ));
        }
        for (kind, heading) in [
            (Kind::Pillar, "Pillars"),
            (Kind::Area, "Capability areas"),
            (Kind::Capability, "Capabilities"),
            (Kind::Level, "Levels"),
        ] {
            let children = model.children(id, kind);
            if children.is_empty() {
                continue;
            }
            body.push_str(&format!("### {heading}\n\n"));
            for child in children {
                let number = &model.nodes[child].number;
                if number.is_empty() {
                    body.push_str(&format!("- {}\n", link(model, child)));
                } else {
                    body.push_str(&format!(
                        "- {} — {}\n",
                        escape(number),
                        link(model, child)
                    ));
                }
            }
            body.push('\n');
        }
        ensure!(
            body.len() <= 512 * 1024,
            "maturity fragment exceeds 512 KiB budget: {id}"
        );
        Ok(body)
    }
}
fn link(model: &Model, id: &NamedNode) -> String {
    format!(
        "[{}]({})",
        escape(&model.nodes[id].label),
        id.as_str()
    )
}
// Source multiline annotation indentation is removed uniformly; relative
// code/list indentation remains intact. Input is bounded by the annotation byte
// budget.
fn dedent(value: &str) -> String {
    let indentation = value
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    let mut output = String::new();
    for line in value.lines() {
        output.push_str(
            &line[indentation
                .min(line.len() - line.trim_start_matches(' ').len())..],
        );
        output.push('\n');
    }
    output.trim().to_owned()
}
#[async_trait]
impl Documentor for MaturityModelDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let model = Model::read(&self.dataset, &self.view()).await?;
        for id in model.order() {
            let body = self.body(&model, &id).await?;
            self.write(&id, &body, writer).await?;
        }
        self.retire(&model, writer).await
    }
}
