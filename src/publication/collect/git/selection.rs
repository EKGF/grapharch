//! Which of a repository's hinted publication targets a family build
//! publishes, and which of them binds the family.
use {
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
};

/// Publication definitions may register at most this many targets.
pub(super) const TARGETS: usize = 16;

/// The hinted targets a family build publishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetSelection {
    /// Every target the hinted project registers, in IRI order.
    All,
    /// The project's only registered target. Several registered targets are
    /// an error that asks for an explicit selection.
    Single,
    /// Explicitly selected registered targets, in the given order.
    Targets(Vec<NamedNode>),
}

impl TargetSelection {
    /// Select explicit targets, rejecting empty, duplicate and oversized
    /// selections.
    pub fn targets(targets: Vec<NamedNode>) -> Result<Self> {
        ensure!(
            !targets.is_empty(),
            "a target selection names at least one target"
        );
        ensure!(
            targets.len() <= TARGETS,
            "a target selection names at most {TARGETS} targets"
        );
        for (index, target) in targets.iter().enumerate() {
            ensure!(
                !targets[..index].contains(target),
                "target {target} is selected twice"
            );
        }
        Ok(Self::Targets(targets))
    }
}

/// Resolved targets of one family build: the target whose profile receives
/// the family bindings, and every selected target in publication order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyTargets {
    selected: Vec<NamedNode>,
    primary:  usize,
}

impl FamilyTargets {
    /// One explicitly configured target that binds the family.
    pub fn single(target: NamedNode) -> Self {
        Self { selected: vec![target], primary: 0 }
    }

    pub(super) fn new(selected: Vec<NamedNode>, primary: usize) -> Self {
        debug_assert!(primary < selected.len());
        Self { selected, primary }
    }

    /// The target whose profile receives the family bindings.
    pub fn primary(&self) -> &NamedNode { &self.selected[self.primary] }

    /// Every selected target, in publication order.
    pub fn selected(&self) -> &[NamedNode] { &self.selected }
}
