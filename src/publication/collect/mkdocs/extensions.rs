use {
    anyhow::{Result, ensure},
    serde_yaml::Value,
};
#[derive(Default)]
pub(super) struct Hints {
    pub roots:        Vec<String>,
    pub dictionaries: Vec<String>,
}
pub(super) fn read(
    value: Option<&Value>,
    plugins: bool,
    errors: &mut Vec<String>,
    hints: &mut Hints,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let mut items = Vec::new(); // bounded: validated YAML node budget.
    match value {
        Value::Sequence(values) => {
            for item in values {
                if let Some(name) = item.as_str() {
                    items.push((name, None));
                } else if let Some(map) = item.as_mapping() {
                    ensure!(
                        map.len() == 1,
                        "extension entry must contain one name"
                    );
                    let (name, options) = map.iter().next().unwrap();
                    items.push((
                        name.as_str().ok_or_else(|| {
                            anyhow::anyhow!("extension name must be text")
                        })?,
                        Some(options),
                    ));
                } else {
                    anyhow::bail!(
                        "extension entry must be a name or options mapping"
                    );
                }
            }
        },
        Value::Mapping(map) => {
            for (name, options) in map {
                items.push((
                    name.as_str().ok_or_else(|| {
                        anyhow::anyhow!("extension name must be text")
                    })?,
                    Some(options),
                ));
            }
        },
        _ => anyhow::bail!("extensions/plugins must be a sequence or mapping"),
    }
    for (name, options) in items {
        let supported = if plugins {
            matches!(
                name,
                "search" | "awesome-pages" | "include-markdown"
            )
        } else {
            matches!(
                name,
                "abbr" |
                    "admonition" |
                    "meta" |
                    "toc" |
                    "tables" |
                    "footnotes" |
                    "fenced_code" |
                    "codehilite" |
                    "attr_list" |
                    "md_in_html" |
                    "pymdownx.details" |
                    "pymdownx.tabbed" |
                    "pymdownx.superfences" |
                    "pymdownx.highlight" |
                    "pymdownx.tasklist" |
                    "pymdownx.snippets"
            )
        };
        if !supported {
            errors.push(format!(
                "{name}: unsupported; no Python extension/plugin is executed"
            ));
            continue;
        }
        if name == "pymdownx.snippets" && !hints.roots.iter().any(|p| p == ".")
        {
            hints.roots.push(".".into());
        }
        if let Some(options) = options.filter(|v| !v.is_null()) {
            let map = options.as_mapping().ok_or_else(|| {
                anyhow::anyhow!("{name} options must be a mapping")
            })?;
            for (key, value) in map {
                let key = key.as_str().ok_or_else(|| {
                    anyhow::anyhow!("{name} option must be text")
                })?;
                if name == "pymdownx.snippets" &&
                    matches!(key, "base_path" | "auto_append")
                {
                    let paths = paths(value)?;
                    if key == "base_path" {
                        ensure!(
                            !paths.is_empty(),
                            "base_path must contain at least one root"
                        );
                        hints.roots = paths;
                    } else {
                        hints.dictionaries = paths;
                    }
                    continue;
                }
                let supported = match (name, key) {
                    ("pymdownx.snippets", "check_paths") => {
                        value == &Value::Bool(true)
                    },
                    ("toc", "permalink") => {
                        value.is_bool() || value.is_string()
                    },
                    ("pymdownx.tabbed", "alternate_style") |
                    ("pymdownx.tasklist", "custom_checkbox") => {
                        value == &Value::Bool(true)
                    },
                    ("codehilite", "css_class") => {
                        value.as_str() == Some("highlight")
                    },
                    _ => false,
                };
                if !supported {
                    errors.push(format!(
                        "{name}.{key}: unsupported option; use explicit \
                         GraphArch configuration"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn paths(value: &Value) -> Result<Vec<String>> {
    let values = value
        .as_sequence()
        .ok_or_else(|| anyhow::anyhow!("snippet paths must be a list"))?;
    ensure!(
        values.len() <= 8,
        "snippet roots and auto_append each allow at most8paths"
    );
    values
        .iter()
        .map(|v| {
            let path = v
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("snippet path must be text"))?;
            ensure!(
                !path.is_empty() &&
                    path.len() <= 2048 &&
                    std::path::Path::new(path).components().all(|c| {
                        matches!(
                            c,
                            std::path::Component::Normal(_) |
                                std::path::Component::CurDir
                        )
                    }),
                "snippet paths must be project-relative without traversal"
            );
            Ok(path.to_owned())
        })
        .collect()
}
