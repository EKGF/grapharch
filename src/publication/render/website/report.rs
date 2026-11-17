//! Generated report pages share the site shell but not the page catalog.
use {
    super::{catalog::Catalog, header::Header, project, shell},
    crate::publication::render::model::Plan,
    anyhow::Result,
    std::path::Path,
};

pub(super) struct Report<'a> {
    pub(super) slug:             &'a str,
    pub(super) title:            &'a str,
    pub(super) body:             &'a [u8],
    pub(super) navigation_owner: Option<&'a str>,
    pub(super) breadcrumb_root:  Option<(&'a str, &'a str)>,
}

impl Report<'_> {
    /// Write the report as `<slug>.html` at the site root.
    pub(super) async fn write(
        self,
        plan: &Plan,
        stage: &Path,
        catalog: &Catalog,
        header: &Header,
    ) -> Result<()> {
        let Report {
            slug,
            title,
            body,
            navigation_owner,
            breadcrumb_root,
        } = self;
        let id = crate::publication::dataset::identity("website-report", &[
            plan.target.as_str(),
            slug,
        ]);
        let navigation = catalog.report_navigation(&id, "./");
        let content = std::str::from_utf8(body)?;
        let html = shell::render(plan, header, shell::Page {
            report: true,
            panels: &[],
            presentation: &project::Presentation::default(),
            extra_head: "<meta name=\"robots\" content=\"noindex,follow\">",
            title,
            id: id.as_str(),
            navigation_owner,
            breadcrumb_root,
            fragment_count: 0,
            prefix: "./",
            navigation: &navigation,
            breadcrumbs: "",
            heading: "",
            body: content,
            gallery: "",
            previous_next: "",
            outline: "",
        })?;
        tokio::fs::write(stage.join(format!("{slug}.html")), html).await?;
        Ok(())
    }
}
