//! Generation evidence read from the pinned source graph.
use {
    super::identifiers::{CommitSha, RepositoryUrl},
    crate::publication::render::model::Plan,
    anyhow::Result,
};

pub(in crate::publication::render) const ID: &str = "document-provenance";

#[derive(Default)]
pub(in crate::publication::render) struct Provenance {
    // bounded: at most 128 source repositories, five commits and 64 tags each.
    pub(super) repositories: Vec<Repository>,
    // bounded: at most sixteen recorded build runs.
    pub(super) operators:    Vec<BuildOperator>,
}

pub(super) struct Repository {
    pub(super) url:          RepositoryUrl,
    pub(super) commit:       CommitSha,
    pub(super) working_tree: bool,
    pub(super) tags:         Vec<String>,
    pub(super) releases:     Vec<(String, String)>,
    pub(super) commits:      Vec<Commit>,
}

pub(super) struct Commit {
    pub(super) id:     CommitSha,
    pub(super) author: String,
    pub(super) time:   chrono::DateTime<chrono::Utc>,
}

/// The recorded activity that ran this publication build, and its agents.
pub(super) struct BuildOperator {
    pub(super) label:  String,
    /// A browsable record of the run, when the activity is an HTTP(S) IRI.
    pub(super) run:    Option<String>,
    // bounded: at most eight associated agents.
    pub(super) agents: Vec<String>,
}

impl Provenance {
    pub(in crate::publication::render) async fn read(
        plan: &Plan,
    ) -> Result<Self> {
        super::read::read(plan).await
    }
}

impl Repository {
    pub(super) fn state(&self) -> &'static str {
        if self.working_tree {
            "Local checkout of this commit; may include uncommitted changes."
        } else {
            "Exact committed revision."
        }
    }

    pub(super) fn commit_url(&self, commit: &CommitSha) -> Option<String> {
        self.url.commit_url(commit)
    }
}

impl Commit {
    pub(super) fn date(&self) -> String {
        self.time.format("%-d %B %Y, %H:%M UTC").to_string()
    }
}
