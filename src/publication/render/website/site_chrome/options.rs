//! Generated website regions a profile may explicitly omit or rearrange.
use {
    super::flag::Flag,
    crate::publication::render::model::Plan,
    anyhow::Result,
};

const PROJECT_NAVIGATION: Flag<'static> = Flag::new("projectNavigation", true);
const SITE_SEARCH: Flag<'static> = Flag::new("siteSearch", true);
const GENERATOR_CREDIT: Flag<'static> = Flag::new("generatorCredit", true);
const BREADCRUMBS: Flag<'static> = Flag::new("breadcrumbs", true);
const PAGE_NEIGHBORS: Flag<'static> = Flag::new("pageNeighbors", true);
const PRIMARY_NAVIGATION: Flag<'static> = Flag::new("primaryNavigation", true);
const OUTLINE_IN_SIDEBAR: Flag<'static> = Flag::new("outlineInSidebar", false);
const SIDEBAR_SECTIONS: Flag<'static> = Flag::new("sidebarSections", false);
const SMART_SYMBOLS: Flag<'static> = Flag::new("smartSymbols", false);

#[derive(Clone, Copy)]
pub(in crate::publication::render::website) struct Options {
    project_navigation: bool,
    search:             bool,
    generator_credit:   bool,
    breadcrumbs:        bool,
    page_neighbors:     bool,
    primary_navigation: bool,
    outline_in_sidebar: bool,
    sidebar_sections:   bool,
    smart_symbols:      bool,
}

impl Options {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let profile = &plan.profile;
        Ok(Self {
            project_navigation: PROJECT_NAVIGATION.read(plan, profile).await?,
            search:             SITE_SEARCH.read(plan, profile).await?,
            generator_credit:   GENERATOR_CREDIT.read(plan, profile).await?,
            breadcrumbs:        BREADCRUMBS.read(plan, profile).await?,
            page_neighbors:     PAGE_NEIGHBORS.read(plan, profile).await?,
            primary_navigation: PRIMARY_NAVIGATION.read(plan, profile).await?,
            outline_in_sidebar: OUTLINE_IN_SIDEBAR.read(plan, profile).await?,
            sidebar_sections:   SIDEBAR_SECTIONS.read(plan, profile).await?,
            smart_symbols:      SMART_SYMBOLS.read(plan, profile).await?,
        })
    }

    /// Whether project pages show the generated page navigation bar.
    pub fn project_navigation(self) -> bool { self.project_navigation }

    /// Whether the site publishes its search index, button and dialog.
    pub fn search(self) -> bool { self.search }

    /// Whether page footers carry the generator credit and provenance link.
    pub fn generator_credit(self) -> bool { self.generator_credit }

    /// Whether pages show the breadcrumb trail.
    pub fn breadcrumbs(self) -> bool { self.breadcrumbs }

    /// Whether pages end with previous and next links.
    pub fn page_neighbors(self) -> bool { self.page_neighbors }

    /// Whether the primary navigation is shown as header tabs.
    pub fn primary_navigation(self) -> bool { self.primary_navigation }

    /// Whether the page outline ends the sidebar instead of following the
    /// page.
    pub fn outline_in_sidebar(self) -> bool { self.outline_in_sidebar }

    /// Whether the sidebar's top-level entries are expanded sections.
    pub fn sidebar_sections(self) -> bool { self.sidebar_sections }

    /// Whether prose presents typographic symbols.
    pub fn smart_symbols(self) -> bool { self.smart_symbols }
}
