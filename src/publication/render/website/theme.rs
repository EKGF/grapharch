//! Source-selected companion website identity; specification authority styling
//! is separate.
use {super::super::model::Plan, anyhow::Result, oxrdf::Term, std::path::Path};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Theme {
    Default,
    Ekgf,
}

impl Theme {
    pub async fn read(plan: &Plan) -> Result<Self> {
        match plan.source.optional(&plan.profile, "style").await? {
            None => Ok(Self::Default),
            Some(Term::Literal(style)) => {
                match style.value() {
                    "reader" => Ok(Self::Default),
                    "ekgf" => Ok(Self::Ekgf),
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

    pub fn css(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ekgf => include_str!("ekgf.css"),
        }
    }

    pub fn body_attribute(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ekgf => " class=\"theme-ekgf\"",
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

    pub fn links(self, repository: &str, prefix: &str) -> String {
        if self == Self::Default {
            return repository.into();
        }
        format!("<div class=\"site-links\"><nav class=\"ekgf-header-navigation\" aria-label=\"EKGF\"><a href=\"https://ekgf.org/about/\">About</a><a href=\"https://ekgf.org/resources/\">Resources</a><a href=\"https://ekgf.org/contact/\">Contact</a></nav>{repository}<a class=\"omg-identity\" href=\"https://www.omg.org/\" aria-label=\"Object Management Group\"><img src=\"{prefix}brand/omg-logo.svg\" alt=\"Object Management Group — Standards Development Organization\"></a></div>")
    }

    pub fn community_footer(self, prefix: &str) -> String {
        if self == Self::Default {
            return String::new();
        }
        let prefix = if prefix.is_empty() { "./" } else { prefix };
        include_str!("community_footer.html")
            .replace("{prefix}", prefix)
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
        // bounded: four fixed, bundled identity/font assets, under 600 KiB
        // combined.
        let assets: [(&str, &[u8]); 4] = [
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
