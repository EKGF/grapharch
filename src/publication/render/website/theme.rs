//! Source-selected companion website identity; specification authority styling
//! is separate.
use {super::super::model::Plan, anyhow::Result, oxrdf::Term, std::path::Path};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Theme {
    Default,
    Ekgf,
    EkgfProject,
}

impl Theme {
    pub async fn read(plan: &Plan) -> Result<Self> {
        match plan.source.optional(&plan.profile, "style").await? {
            None => Ok(Self::Default),
            Some(Term::Literal(style)) => {
                match style.value() {
                    "reader" => Ok(Self::Default),
                    "ekgf" => Ok(Self::Ekgf),
                    "ekgf-project" => Ok(Self::EkgfProject),
                    value => {
                        anyhow::bail!(
                            "unsupported website presentation style {value}"
                        )
                    },
                }
            },
            _ => anyhow::bail!("website presentation style must be a literal"),
        }
    }

    pub fn project(self) -> bool { self == Self::EkgfProject }

    pub fn community(self) -> bool { self != Self::Default }

    pub fn css(self) -> &'static str {
        match self {
            Self::Default => include_str!("project.css"),
            Self::Ekgf => {
                concat!(
                    include_str!("ekgf.css"),
                    "\n",
                    include_str!("project.css"),
                    "\n",
                    include_str!("color_scheme.css")
                )
            },
            Self::EkgfProject => {
                concat!(
                    include_str!("ekgf.css"),
                    "\n",
                    include_str!("project.css"),
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
        }
    }

    pub fn identity(self) -> String {
        if self == Self::Default {
            return String::new();
        }
        format!(
            "<a class=\"ekgf-identity\" href=\"https://ekgf.org/\" \
             aria-label=\"EKGF home\">{}<span>EKGF</span></a>",
            include_str!("brand/ekgf-mark.svg")
        )
    }

    pub fn links(
        self,
        repository: &str,
        prefix: &str,
        project_links: &str,
    ) -> String {
        if self == Self::Default {
            return repository.into();
        }
        format!("<details class=\"community-menu\" open><summary aria-label=\"Toggle community navigation\">Menu</summary><nav class=\"ekgf-header-navigation\" aria-label=\"EKGF\"><a href=\"https://ekgf.org/about/\">About</a><a href=\"https://ekgf.org/quadrants/\">Quadrants</a><a href=\"https://ekgf.org/resources/\">Resources</a><a href=\"https://ekgf.org/membership/\">Membership</a><a href=\"https://ekgf.org/contact/\">Contact</a>{project_links}</nav></details><div class=\"site-links\"><button class=\"color-scheme-toggle\" type=\"button\" aria-label=\"Dark theme\" aria-pressed=\"false\" title=\"Use dark theme\"><svg class=\"theme-moon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><path d=\"M20.5 13.3A8.5 8.5 0 0 1 10.7 3.5 8.5 8.5 0 1 0 20.5 13.3Z\"/></svg><svg class=\"theme-sun\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"4\"/><path d=\"M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5\"/></svg></button>{repository}<a class=\"omg-identity\" href=\"https://www.omg.org/\" aria-label=\"Object Management Group\"><img src=\"{prefix}brand/omg-logo.svg\" alt=\"Object Management Group — Standards Development Organization\"></a></div>")
    }

    pub fn community_footer(self, prefix: &str) -> String {
        if self == Self::Default {
            return String::new();
        }
        let home = if prefix.is_empty() { "./" } else { prefix };
        include_str!("community_footer.html")
            .replace("{prefix}", prefix)
            .replace("{home}", home)
            .replace("{generator_credit}", super::super::credit::HTML)
    }

    pub fn attribution(self, prefix: &str) -> String {
        if self == Self::Default {
            return String::new();
        }
        format!(
            " · <a href=\"{prefix}brand/NOTICE.txt\">EKGF identity credits</a>"
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
        // bounded: five fixed, bundled identity/font/license assets, under 600
        // KiB combined.
        let assets: [(&str, &[u8]); 5] = [
            (
                "brand/roboto.ttf",
                include_bytes!("brand/roboto.ttf"),
            ),
            (
                "brand/omg-logo.svg",
                include_bytes!("brand/omg-logo.svg"),
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
