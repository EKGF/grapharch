//! The generated regions one page hides, such as its sidebar or outline.
use {
    crate::publication::{render::model::Plan, term},
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};

#[derive(Clone, Copy, Default)]
pub(super) struct PageRegions {
    navigation: bool,
    outline:    bool,
}

impl PageRegions {
    pub(super) async fn read(plan: &Plan, page: &NamedNode) -> Result<Self> {
        let mut regions = Self::default();
        let mut rows = plan.source.objects(page, "hiddenRegion").await?;
        while let Some(row) = rows.next().await? {
            match &row["value"] {
                Term::NamedNode(region)
                    if *region == term("NavigationRegion") =>
                {
                    regions.navigation = true;
                },
                Term::NamedNode(region) if *region == term("OutlineRegion") => {
                    regions.outline = true;
                },
                other => bail!("{page} hides unknown page region {other}"),
            }
        }
        Ok(regions)
    }

    /// Whether the page hides its navigation sidebar.
    pub(super) fn navigation(self) -> bool { self.navigation }

    /// Whether the page hides its outline of headings.
    pub(super) fn outline(self) -> bool { self.outline }
}
