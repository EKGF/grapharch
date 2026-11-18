//! Generated website regions a profile may explicitly omit.
use {
    super::flag::Flag,
    crate::publication::render::model::Plan,
    anyhow::Result,
};

const PROJECT_NAVIGATION: Flag<'static> = Flag::new("projectNavigation", true);
const SITE_SEARCH: Flag<'static> = Flag::new("siteSearch", true);
const GENERATOR_CREDIT: Flag<'static> = Flag::new("generatorCredit", true);

#[derive(Clone, Copy)]
pub(in crate::publication::render::website) struct Options {
    project_navigation: bool,
    search:             bool,
    generator_credit:   bool,
}

impl Options {
    pub async fn read(plan: &Plan) -> Result<Self> {
        Ok(Self {
            project_navigation: PROJECT_NAVIGATION
                .read(plan, &plan.profile)
                .await?,
            search:             SITE_SEARCH.read(plan, &plan.profile).await?,
            generator_credit:   GENERATOR_CREDIT
                .read(plan, &plan.profile)
                .await?,
        })
    }

    /// Whether project pages show the generated page navigation bar.
    pub fn project_navigation(self) -> bool { self.project_navigation }

    /// Whether the site publishes its search index, button and dialog.
    pub fn search(self) -> bool { self.search }

    /// Whether page footers carry the generator credit and provenance link.
    pub fn generator_credit(self) -> bool { self.generator_credit }
}
