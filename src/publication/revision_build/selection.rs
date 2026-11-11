//! Target selection: explicit targets in their given order, or every target
//! the definitions register, in IRI order.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Serialize, Serializer},
};

const TARGETS: usize = 16;

/// The targets a revision build resolves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetSelection {
    /// Every target registered by the project definitions, in IRI order.
    All,
    /// Explicitly selected registered targets, in the given order.
    Targets(Vec<NamedNode>),
}

impl TargetSelection {
    /// Select explicit targets, rejecting empty, duplicate and oversized
    /// selections.
    pub fn targets(targets: Vec<NamedNode>) -> Result<Self> {
        ensure!(
            !targets.is_empty(),
            "at least one target is required"
        );
        ensure!(
            targets.len() <= TARGETS,
            "a revision build selects at most {TARGETS} targets"
        );
        for (index, target) in targets.iter().enumerate() {
            ensure!(
                !targets[..index].contains(target),
                "target {target} is selected twice"
            );
        }
        Ok(Self::Targets(targets))
    }

    /// Resolve the selection against the definitions only.
    pub(super) async fn resolve(
        &self,
        definitions: &RevisionView,
        project: &NamedNode,
    ) -> Result<Vec<NamedNode>> {
        let mut registered = Vec::new(); // bounded: TARGETS.
        let mut rows = definitions.objects(project, "target").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                registered.len() < TARGETS,
                "{project} registers more than {TARGETS} targets"
            );
            let Term::NamedNode(target) = &row["value"] else {
                bail!("{project} ga:target must identify an IRI")
            };
            registered.push(target.clone());
        }
        registered.sort();
        match self {
            Self::All => {
                ensure!(
                    !registered.is_empty(),
                    "{project} registers no publication targets"
                );
                Ok(registered)
            },
            Self::Targets(targets) => {
                for target in targets {
                    ensure!(
                        registered.contains(target),
                        "target {target} is not registered by {project}"
                    );
                }
                Ok(targets.clone())
            },
        }
    }
}

impl Serialize for TargetSelection {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.serialize_str("all"),
            Self::Targets(targets) => {
                serializer.collect_seq(targets.iter().map(NamedNode::as_str))
            },
        }
    }
}
