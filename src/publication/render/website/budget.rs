//! Artifact inventory bounds derived from the independent semantic producers.
use anyhow::{Result, ensure};
pub(super) const PAGES: usize = 256;
pub(super) const ASSETS: usize = crate::publication::limits::WEBSITE_ASSETS;
pub(super) const PROJECTIONS: usize = 256;
pub(super) const ALIASES: usize = 2048;
// Three base files, five bundled identity/font files, two crawler indexes,
// three reports (quality, provenance, diagrams), one publication inventory, and
// two search files.
const SHELL: usize = 3 + 5 + 2 + 3 + 1 + 2;
pub(super) const MAX_ARTIFACTS: usize =
    // The common publisher adds its icon license after website generation.
    PAGES + ASSETS + PROJECTIONS + ALIASES + SHELL + 1;
pub(super) struct ArtifactBudget {
    selected: usize,
}
impl ArtifactBudget {
    pub fn new(
        pages: usize,
        assets: usize,
        projections: usize,
    ) -> Result<Self> {
        ensure!(
            pages <= PAGES,
            "website exceeds {PAGES}-page budget"
        );
        ensure!(
            assets <= ASSETS,
            "website exceeds {ASSETS}-asset budget"
        );
        ensure!(
            projections <= PROJECTIONS,
            "website exceeds {PROJECTIONS}-projection budget"
        );
        Ok(Self { selected: pages + assets + projections })
    }

    pub fn validate(&self, aliases: usize, total: usize) -> Result<()> {
        ensure!(
            aliases <= ALIASES,
            "website exceeds {ALIASES} route aliases"
        );
        let semantic = self.selected + aliases;
        let shell = total.checked_sub(semantic).ok_or_else(|| {
            anyhow::anyhow!("website artifact inventory omits selected content")
        })?;
        ensure!(
            shell <= SHELL,
            "website exceeds {SHELL} generated shell artifacts"
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_and_projected_representations_have_distinct_allowances()
    -> Result<()> {
        // The real Catalog has 210 authored pages and 193 interactive diagrams.
        ArtifactBudget::new(210, 193, 193)?
            .validate(0, 210 + 193 + 193 + SHELL)?;
        ArtifactBudget::new(PAGES, ASSETS, PROJECTIONS)?.validate(
            ALIASES,
            PAGES + ASSETS + PROJECTIONS + ALIASES + SHELL,
        )?;
        Ok(())
    }
    #[test]
    fn every_producer_keeps_its_existing_hard_bound() {
        assert!(ArtifactBudget::new(PAGES + 1, 0, 0).is_err());
        assert!(ArtifactBudget::new(0, ASSETS + 1, 0).is_err());
        assert!(ArtifactBudget::new(0, 0, PROJECTIONS + 1).is_err());
        let budget = ArtifactBudget::new(1, 1, 1).unwrap();
        assert!(budget.validate(ALIASES + 1, ALIASES + 4).is_err());
        assert!(budget.validate(0, 3 + SHELL + 1).is_err());
        assert!(budget.validate(0, 2).is_err());
    }
}
