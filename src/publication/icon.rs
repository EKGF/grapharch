//! A fixed, bundled decorative icon vocabulary; no source SVG or remote lookup.
use anyhow::{Result, ensure};
const PREFIX: &str = "urn:grapharch:icon:";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Name {
    ArrowRight,
    Cached,
    FloorPlan,
    Run,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Icon {
    pub name:  Name,
    pub large: bool,
}
impl Icon {
    pub fn from_source(name: &str, large: bool) -> Result<Self> {
        let name = match name {
            "octicons-arrow-right-24" => Name::ArrowRight,
            "material-cached" => Name::Cached,
            "material-floor-plan" => Name::FloorPlan,
            "material-run" => Name::Run,
            _ => anyhow::bail!("unsupported decorative icon: {name}"),
        };
        Ok(Self { name, large })
    }

    pub fn parse_uri(uri: &str) -> Result<Option<Self>> {
        let Some(value) = uri.strip_prefix(PREFIX) else {
            return Ok(None)
        };
        let (name, large) = value
            .strip_suffix(":large")
            .map_or((value, false), |name| (name, true));
        let name = match name {
            "arrow-right" => Name::ArrowRight,
            "cached" => Name::Cached,
            "floor-plan" => Name::FloorPlan,
            "run" => Name::Run,
            _ => anyhow::bail!("unknown bundled decorative icon: {name}"),
        };
        Ok(Some(Self { name, large }))
    }

    pub fn uri(self) -> String {
        format!(
            "{PREFIX}{}{}",
            self.name(),
            if self.large { ":large" } else { "" }
        )
    }

    pub fn name(self) -> &'static str {
        match self.name {
            Name::ArrowRight => "arrow-right",
            Name::Cached => "cached",
            Name::FloorPlan => "floor-plan",
            Name::Run => "run",
        }
    }

    pub fn svg(self) -> &'static str {
        match self.name {
            Name::ArrowRight => {
                include_str!("render/website/project/icons/arrow-right.svg")
            },
            Name::Cached => {
                include_str!("render/website/project/icons/material/cached.svg")
            },
            Name::FloorPlan => {
                include_str!(
                    "render/website/project/icons/material/floor-plan.svg"
                )
            },
            Name::Run => {
                include_str!("render/website/project/icons/material/run.svg")
            },
        }
    }

    pub fn html(self) -> Result<String> {
        ensure!(
            self.svg().starts_with("<svg "),
            "bundled icon must be an SVG root"
        );
        Ok(self.svg().replacen(
            "<svg ",
            &format!(
                "<svg class=\"inline-icon{}\" aria-hidden=\"true\" \
                 focusable=\"false\" ",
                if self.large { " inline-icon-large" } else { "" }
            ),
            1,
        ))
    }
}
pub(crate) async fn write_licenses(root: &std::path::Path) -> Result<String> {
    const NAME: &str = "grapharch-icon-licenses.txt";
    let path = root.join(NAME);
    ensure!(
        !tokio::fs::try_exists(&path).await?,
        "icon license artifact collides with publication output"
    );
    let notices = concat!(
        "GraphArch bundled icons\n\nLucide / Feather\n",
        include_str!("render/website/project/icons/LICENSE"),
        "\nMaterial Design Icons\n",
        include_str!("render/website/project/icons/material/README.md"),
        "\n",
        include_str!("render/website/project/icons/material/LICENSE"),
        "\n",
        include_str!(
            "render/website/project/icons/material/LICENSE-APACHE-2.0"
        )
    );
    tokio::fs::write(path, notices).await?;
    Ok(NAME.into())
}
#[cfg(test)]
mod tests {
    use super::Icon;
    #[test]
    fn fixed_icons_are_decorative_and_round_trip_without_external_assets() {
        for name in [
            "octicons-arrow-right-24",
            "material-cached",
            "material-floor-plan",
            "material-run",
        ] {
            let icon = Icon::from_source(name, true).unwrap();
            assert_eq!(Icon::parse_uri(&icon.uri()).unwrap(), Some(icon));
            assert!(icon.html().unwrap().contains("aria-hidden=\"true\""));
            assert!(!icon.svg().contains("<script"));
        }
        assert!(Icon::from_source("material-unknown", false).is_err());
        assert!(Icon::parse_uri("urn:grapharch:icon:unknown").is_err());
    }
}
