//! Material for MkDocs presentation options read as data: `theme.features`,
//! `theme.palette`, `extra.generator`, `extra.social` and `copyright`. They
//! become profile switches, theme tokens and footer content; nothing is
//! executed.
use {
    super::warning::ImportWarning,
    anyhow::{Result, ensure},
    serde_yaml::{Mapping, Value},
    std::collections::BTreeSet,
};

/// Most social links a footer presents.
const MAX_SOCIAL: usize = 16;

/// A `theme.features` flag of Material for MkDocs that changes the layout.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Feature {
    /// Top-level sections as header tabs.
    Tabs,
    /// Top-level sidebar entries as expanded sections.
    Sections,
    /// Section index pages; GraphArch links sections to their index pages.
    Indexes,
    /// The page outline integrated into the navigation sidebar.
    TocIntegrate,
    /// Breadcrumbs above the page title.
    Path,
    /// Previous and next links at the end of every page.
    Footer,
}

impl Feature {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "navigation.tabs" => Self::Tabs,
            "navigation.sections" => Self::Sections,
            "navigation.indexes" => Self::Indexes,
            "toc.integrate" => Self::TocIntegrate,
            "navigation.path" => Self::Path,
            "navigation.footer" => Self::Footer,
            _ => return None,
        })
    }
}

/// One `extra.social` entry.
pub(super) struct SocialLink {
    pub icon:  String,
    pub link:  String,
    pub title: Option<String>,
}

/// Material presentation options of one configuration.
#[derive(Default)]
pub(super) struct Material {
    // bounded: the fixed feature set.
    pub features:  BTreeSet<Feature>,
    /// The palette's primary colour name of its first scheme.
    pub primary:   Option<String>,
    /// Whether `extra.generator` asks for the generator credit.
    pub generator: bool,
    // bounded: at most MAX_SOCIAL links.
    pub social:    Vec<SocialLink>,
    pub copyright: Option<String>,
}

impl Material {
    pub(super) fn read(
        map: &Mapping,
        warnings: &mut Vec<ImportWarning>,
    ) -> Result<Self> {
        let mut material = Self::default();
        let theme = map.get("theme").and_then(Value::as_mapping);
        if let Some(features) = theme.and_then(|t| t.get("features")) {
            let features = features.as_sequence().ok_or_else(|| {
                anyhow::anyhow!("theme.features must be a list")
            })?;
            for feature in features {
                let name = feature.as_str().ok_or_else(|| {
                    anyhow::anyhow!("theme.features entries must be text")
                })?;
                match Feature::named(name) {
                    Some(feature) => {
                        material.features.insert(feature);
                    },
                    None => {
                        warnings.push(ImportWarning::substituted(format!(
                            "theme.features {name}: not reproduced; GraphArch \
                             provides its own page behaviour"
                        )));
                    },
                }
            }
        }
        if let Some(palette) = theme.and_then(|t| t.get("palette")) {
            let first = palette.as_sequence().and_then(|s| s.first());
            material.primary = first
                .unwrap_or(palette)
                .get("primary")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let extra = map.get("extra").and_then(Value::as_mapping);
        material.generator = extra
            .and_then(|e| e.get("generator"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(social) = extra.and_then(|e| e.get("social")) {
            let social = social.as_sequence().ok_or_else(|| {
                anyhow::anyhow!("extra.social must be a list")
            })?;
            ensure!(
                social.len() <= MAX_SOCIAL,
                "extra.social allows at most {MAX_SOCIAL} links"
            );
            for entry in social {
                let field = |key: &str| {
                    entry.get(key).and_then(Value::as_str).map(str::to_owned)
                };
                material.social.push(SocialLink {
                    icon:  field("icon").ok_or_else(|| {
                        anyhow::anyhow!("extra.social entries require icon")
                    })?,
                    link:  field("link").ok_or_else(|| {
                        anyhow::anyhow!("extra.social entries require link")
                    })?,
                    title: field("name"),
                });
            }
        }
        material.copyright = map
            .get("copyright")
            .and_then(Value::as_str)
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        Ok(material)
    }

    pub(super) fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}
