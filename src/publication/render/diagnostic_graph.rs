use {
    super::model::Plan,
    crate::publication::{
        diagnostic_graph::{self, Context},
        diagnostics::Diagnostic,
    },
    anyhow::Result,
    oxrdf::{NamedNode, Quad},
};
pub(super) fn quads(
    plan: &Plan,
    job: &str,
    errors: &[Diagnostic],
) -> Result<Vec<Quad>> {
    let context = Context {
        graph:     NamedNode::new_unchecked("urn:grapharch:publications"),
        id:        NamedNode::new(format!("urn:grapharch:publication:{job}"))?,
        project:   plan.project.clone(),
        about:     plan.target.clone(),
        revision:  plan.revision.clone(),
        processor: NamedNode::new(format!(
            "urn:grapharch:renderer:{}",
            plan.renderer
        ))?,
        source:    plan.handle.id.as_str().to_owned(),
    };
    diagnostic_graph::quads(&context, job, errors)
}
