//! Site-wide presentation settings of an MkDocs configuration: `extra`,
//! `extra_css`, `extra_javascript` and `hooks`. Stylesheets and scripts are
//! never injected and hooks never run; each entry yields a typed diagnostic
//! that names the native GraphArch equivalent where one exists.
use {
    super::warning::ImportWarning,
    anyhow::{Result, ensure},
    serde_yaml::{Mapping, Value},
};

/// A browser script whose observed behavior GraphArch provides natively.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KnownScript {
    /// Swaps `#darkable` SVG objects to their `_dark.svg` variant.
    DarkableObjects,
    /// Shows a page's `letter_prefix` front matter as a heading badge.
    ObjectiveBadges,
    /// Sets Mermaid theme variables for Material's Mermaid integration.
    MermaidConfiguration,
    /// Renders graphviz fences in the browser.
    Graphviz,
}
impl KnownScript {
    fn of(path: &str) -> Option<Self> {
        let file = path.rsplit('/').next()?;
        Some(match file {
            "darkable-objects.js" => Self::DarkableObjects,
            "objective-badges.js" => Self::ObjectiveBadges,
            "mermaid.js" => Self::MermaidConfiguration,
            "mkdocs-graphviz.js" => Self::Graphviz,
            _ => return None,
        })
    }

    const fn native(self) -> &'static str {
        match self {
            Self::DarkableObjects => {
                "#darkable image and object fragments select the _dark.svg \
                 variant natively"
            },
            Self::ObjectiveBadges => {
                "letter_prefix front matter is presented as a heading badge"
            },
            Self::MermaidConfiguration => {
                "Mermaid fences render with GraphArch's theme colors"
            },
            Self::Graphviz => "graphviz fences are kept as highlighted source",
        }
    }
}

pub(super) struct Presentation;
impl Presentation {
    pub(super) fn read(map: &Mapping) -> Result<Vec<ImportWarning>> {
        // bounded: one diagnostic per listed entry of a 4096-node document.
        let mut warnings = Vec::new();
        if let Some(extra) = map.get("extra") {
            let extra = extra
                .as_mapping()
                .ok_or_else(|| anyhow::anyhow!("extra must be a mapping"))?;
            for key in extra.keys() {
                let key = key.as_str().ok_or_else(|| {
                    anyhow::anyhow!("extra keys must be text")
                })?;
                warnings.push(ImportWarning::substituted(format!(
                    "extra.{key}: template variables, social links and \
                     analytics are not reproduced; nothing is injected"
                )));
            }
        }
        for path in entries(map, "extra_javascript")? {
            warnings.push(match KnownScript::of(&path) {
                Some(script) => {
                    ImportWarning::native(format!(
                        "extra_javascript {path}: not injected; {}",
                        script.native()
                    ))
                },
                None => {
                    ImportWarning::not_executed(format!(
                        "extra_javascript {path}: arbitrary scripts are never \
                         injected"
                    ))
                },
            });
        }
        for path in entries(map, "extra_css")? {
            warnings.push(ImportWarning::substituted(format!(
                "extra_css {path}: not injected; GraphArch's theme presents \
                 the page"
            )));
        }
        for path in entries(map, "hooks")? {
            warnings.push(ImportWarning::not_executed(format!(
                "hooks {path}: Python hooks are never executed"
            )));
        }
        Ok(warnings)
    }
}

fn entries(map: &Mapping, key: &str) -> Result<Vec<String>> {
    let Some(value) = map.get(key) else {
        return Ok(Vec::new())
    };
    let values = value
        .as_sequence()
        .ok_or_else(|| anyhow::anyhow!("{key} must be a list"))?;
    ensure!(
        values.len() <= 64,
        "{key} allows at most 64 entries"
    );
    values
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .or_else(|| entry.get("path").and_then(Value::as_str))
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("{key} entries must be paths"))
        })
        .collect()
}
