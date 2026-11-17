//! Generated website regions a profile may explicitly omit.
use {
    super::flag::Flag,
    crate::publication::render::model::Plan,
    anyhow::Result,
};

const PROJECT_NAVIGATION: Flag<'static> = Flag::new("projectNavigation", true);
const SITE_SEARCH: Flag<'static> = Flag::new("siteSearch", true);

#[derive(Clone, Copy)]
pub(in crate::publication::render::website) struct Options {
    project_navigation: bool,
    search:             bool,
}

impl Options {
    pub async fn read(plan: &Plan) -> Result<Self> {
        Ok(Self {
            project_navigation: PROJECT_NAVIGATION
                .read(plan, &plan.profile)
                .await?,
            search:             SITE_SEARCH.read(plan, &plan.profile).await?,
        })
    }

    /// Whether project pages show the generated page navigation bar.
    pub fn project_navigation(self) -> bool { self.project_navigation }

    /// Whether the site publishes its search index, button and dialog.
    pub fn search(self) -> bool { self.search }
}
