//! The configurable number of sources one discovery may select.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

/// Project property that raises or lowers the discovery budget.
pub(crate) const SETTING: &str = "maxDiscoveredSources";

/// The most sources one discovery selects before failing with an error that
/// names [`SETTING`]. Paths are retained sorted, so the budget bounds memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceBudget(usize);

impl SourceBudget {
    /// Large enough for real repositories, small enough to stay in one
    /// publication's entry budget.
    pub(crate) const DEFAULT: Self =
        Self(crate::publication::limits::PUBLICATION_ENTRIES as usize / 4);

    pub(crate) fn try_new(value: u64) -> Result<Self> {
        ensure!(
            (1..=crate::publication::limits::PUBLICATION_ENTRIES)
                .contains(&value),
            "ga:{SETTING} must be between 1 and {}",
            crate::publication::limits::PUBLICATION_ENTRIES
        );
        Ok(Self(value as usize))
    }

    /// The budget `project` declares, else the default.
    pub(crate) async fn read(
        view: &RevisionView,
        project: &NamedNode,
    ) -> Result<Self> {
        match view.optional(project, SETTING).await? {
            None => Ok(Self::DEFAULT),
            Some(Term::Literal(value)) => {
                Self::try_new(value.value().parse().map_err(|_| {
                    anyhow::anyhow!("ga:{SETTING} must be a positive integer")
                })?)
            },
            Some(_) => bail!("ga:{SETTING} must be an integer literal"),
        }
    }

    /// Reject one more source beyond the budget, naming the setting.
    pub(crate) fn admit(self, selected: usize, root: &str) -> Result<()> {
        ensure!(
            selected < self.0,
            "source discovery in {root} exceeds {} sources; raise \
             ga:{SETTING} on the project (or --max-sources for grapharch \
             build), or exclude paths with ga:excludeSourcePattern",
            self.0
        );
        Ok(())
    }
}

impl Default for SourceBudget {
    fn default() -> Self { Self::DEFAULT }
}
