//! The bundled website skin a profile's theme lineage selects: stylesheet,
//! colour scheme support and font. Organization identity, header links and
//! footers are RDF on the profile or its themes; see `site_chrome`.
use {
    super::super::model::Plan,
    crate::publication::theme::{Builtin, Lineage},
    anyhow::Result,
    std::path::Path,
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Theme {
    Default,
    Ekgf,
    EkgfProject,
    /// EKGF's documentation sites: identity row, tabs, EKGF footer.
    EkgfDocumentation,
}

impl Theme {
    pub async fn read(plan: &Plan) -> Result<Self> {
        Ok(
            match Lineage::read(&plan.source, &plan.profile, Builtin::Reader)
                .await?
                .builtin()
            {
                Builtin::Reader |
                Builtin::Specification |
                Builtin::Overview => Self::Default,
                Builtin::Ekgf => Self::Ekgf,
                Builtin::EkgfProject => Self::EkgfProject,
                Builtin::EkgfDocumentation => Self::EkgfDocumentation,
            },
        )
    }

    /// The selector of the skin's root, where theme tokens apply.
    pub fn root(self) -> &'static str {
        if self.community() {
            ".theme-ekgf"
        } else {
            ":root"
        }
    }

    pub fn project(self) -> bool { self == Self::EkgfProject }

    pub fn community(self) -> bool { self != Self::Default }

    /// Whether the header is the documentation identity row: brand logo,
    /// centred site title and identity marks above the tabs.
    pub fn documentation_identity(self) -> bool {
        self == Self::EkgfDocumentation
    }

    pub fn css(self) -> &'static str {
        match self {
            Self::Default => {
                concat!(
                    include_str!("project.css"),
                    "\n",
                    include_str!("shell.css")
                )
            },
            Self::Ekgf => {
                concat!(
                    include_str!("ekgf.css"),
                    "\n",
                    include_str!("project.css"),
                    "\n",
                    include_str!("color_scheme.css")
                )
            },
            Self::EkgfDocumentation => {
                concat!(
                    include_str!("ekgf.css"),
                    "\n",
                    include_str!("project.css"),
                    "\n",
                    include_str!("color_scheme.css"),
                    "\n",
                    include_str!("ekgf_documentation.css")
                )
            },
            Self::EkgfProject => {
                concat!(
                    include_str!("ekgf.css"),
                    "\n",
                    include_str!("project.css"),
                    "\n",
                    include_str!("ekgf_project.css"),
                    "\n",
                    include_str!("color_scheme.css")
                )
            },
        }
    }

    pub fn prepaint(self) -> String {
        if !self.community() {
            return String::new();
        }
        format!(
            "<meta name=\"color-scheme\" content=\"light dark\"><script \
             data-color-scheme-controller>{}</script>",
            include_str!("color_scheme.js")
        )
    }

    pub fn body_attribute(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ekgf => " class=\"theme-ekgf\"",
            Self::EkgfProject => " class=\"theme-ekgf project-website\"",
            Self::EkgfDocumentation => {
                " class=\"theme-ekgf ekgf-documentation\""
            },
        }
    }

    /// The header's controls: an optional menu of header links labelled
    /// for `organization`, then search, the colour scheme toggle of skins
    /// supporting it, the repository and identity marks.
    pub fn links(
        self,
        organization: Option<&str>,
        header_links: &str,
        controls: Controls<'_>,
    ) -> String {
        let menu = if header_links.is_empty() {
            String::new()
        } else {
            format!(
                "<details class=\"community-menu\" open><summary \
                 aria-label=\"Toggle site navigation\">Menu</summary><nav \
                 class=\"header-navigation\" \
                 aria-label=\"{}\">{header_links}</nav></details>",
                super::super::markup::html_escape(
                    organization.unwrap_or("Site")
                )
            )
        };
        let toggle = if self.community() { COLOR_TOGGLE } else { "" };
        format!(
            "{menu}<div class=\"site-links\">{}{toggle}{}{}</div>",
            controls.search, controls.repository, controls.marks
        )
    }

    pub async fn assets(
        self,
        stage: &Path,
        files: &mut Vec<String>,
    ) -> Result<()> {
        if self == Self::Default {
            return Ok(());
        }
        tokio::fs::create_dir_all(stage.join("brand")).await?;
        // bounded: four fixed, bundled font and license assets.
        let assets: [(&str, &[u8]); 4] = [
            (
                "brand/roboto.ttf",
                include_bytes!("brand/roboto.ttf"),
            ),
            ("brand/OFL.txt", include_bytes!("brand/OFL.txt")),
            (
                "brand/LUCIDE-LICENSE.txt",
                include_bytes!("project/icons/LICENSE"),
            ),
            (
                "brand/NOTICE.txt",
                include_bytes!("brand/NOTICE.txt"),
            ),
        ];
        for (path, bytes) in assets {
            tokio::fs::write(stage.join(path), bytes).await?;
            files.push(path.into());
        }
        Ok(())
    }
}

/// The header controls every skin places after its links.
pub(super) struct Controls<'a> {
    pub search:     &'a str,
    pub repository: &'a str,
    pub marks:      &'a str,
}

pub(super) const COLOR_TOGGLE: &str =
    "<button class=\"color-scheme-toggle\" type=\"button\" aria-label=\"Dark \
     theme\" aria-pressed=\"false\" title=\"Use dark theme\"><svg \
     class=\"theme-moon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><path \
     d=\"M20.5 13.3A8.5 8.5 0 0 1 10.7 3.5 8.5 8.5 0 1 0 20.5 \
     13.3Z\"/></svg><svg class=\"theme-sun\" viewBox=\"0 0 24 24\" \
     aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"4\"/><path d=\"M12 \
     2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 \
     5\"/></svg></button>";
