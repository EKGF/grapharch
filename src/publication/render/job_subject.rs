//! What a publication job records about: one resolved plan of one target,
//! shared by every renderer, whether it arranges a scope or publishes files.
use {
    crate::publication::{Renderer, dataset::Dataset, plan::ResolvedPlan},
    oxrdf::NamedNode,
};

pub(super) struct JobSubject {
    pub dataset:  Dataset,
    pub handle:   ResolvedPlan,
    pub project:  NamedNode,
    pub target:   NamedNode,
    pub revision: NamedNode,
    pub renderer: Renderer,
}

impl super::model::Plan {
    /// The recorded identity of this plan's publication jobs.
    pub(super) fn job_subject(&self) -> JobSubject {
        JobSubject {
            dataset:  self.dataset.clone(),
            handle:   self.handle.clone(),
            project:  self.project.clone(),
            target:   self.target.clone(),
            revision: self.revision.clone(),
            renderer: self.renderer,
        }
    }
}
