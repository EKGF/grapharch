use {
    anyhow::{Result, ensure},
    serde_yaml::{Mapping, Value},
    std::collections::BTreeSet,
};

pub(super) struct Config {
    pub title:       String,
    pub docs:        String,
    pub description: Option<String>,
    pub repository:  Option<String>,
    pub site_url:    Option<String>,
    pub nav:         Option<Value>,
    pub warnings:    Vec<String>,
}
impl Config {
    pub fn parse(body: &str) -> Result<Self> {
        ensure!(
            body.len() <= 128 * 1024,
            "MkDocs configuration exceeds 128 KiB"
        );
        reject_tags(body)?;
        let value: Value = serde_yaml::from_str(body)?;
        let mut count = 0;
        safe(&value, 0, &mut count)?;
        let map = value.as_mapping().ok_or_else(|| {
            anyhow::anyhow!("MkDocs configuration must be a mapping")
        })?;
        let mut errors = Vec::new(); // bounded: YAML node limit 4096.
        let mut warnings = Vec::new(); // bounded: same node limit.
        let supported: BTreeSet<&str> = [
            "site_name",
            "site_description",
            "site_url",
            "site_author",
            "repo_url",
            "repo_name",
            "edit_uri",
            "copyright",
            "docs_dir",
            "site_dir",
            "nav",
            "theme",
            "plugins",
            "markdown_extensions",
            "strict",
            "dev_addr",
            "watch",
            "use_directory_urls",
            "validation",
        ]
        .into_iter()
        .collect();
        for key in map.keys() {
            let key = key.as_str().ok_or_else(|| {
                anyhow::anyhow!("MkDocs option names must be strings")
            })?;
            if !supported.contains(key) {
                errors.push(format!(
                    "{key}: unsupported; remove it or supply explicit \
                     GraphArch configuration"
                ));
            }
        }
        for key in [
            "site_author",
            "repo_name",
            "edit_uri",
            "copyright",
            "site_dir",
            "strict",
            "dev_addr",
            "watch",
            "validation",
        ] {
            if map.contains_key(key) {
                warnings.push(format!(
                    "{key}: retained in the source configuration; GraphArch \
                     uses its own publication, validation, and output settings"
                ));
            }
        }
        if map
            .get("use_directory_urls")
            .is_some_and(|v| v == &Value::Bool(false))
        {
            errors.push(
                "use_directory_urls: false is not supported; GraphArch uses \
                 directory routes"
                    .into(),
            );
        }
        extensions(map.get("markdown_extensions"), false, &mut errors)?;
        extensions(map.get("plugins"), true, &mut errors)?;
        if let Some(theme) = map.get("theme") {
            let name = theme
                .as_str()
                .or_else(|| theme.get("name").and_then(Value::as_str))
                .unwrap_or("custom");
            warnings.push(format!(
                "MkDocs theme {name:?} is presented with GraphArch's shared \
                 documentation theme; Python/Jinja/CSS is not imported"
            ));
            if let Some(theme) = theme.as_mapping() {
                for key in theme.keys() {
                    let key = key.as_str().ok_or_else(|| {
                        anyhow::anyhow!("theme keys must be strings")
                    })?;
                    if matches!(key, "custom_dir" | "static_templates") {
                        errors.push(format!(
                            "theme.{key}: custom theme execution is not \
                             supported"
                        ));
                    } else if key != "name" {
                        warnings.push(format!(
                            "theme.{key}: retained as source metadata; \
                             GraphArch controls this presentation setting"
                        ));
                    }
                }
            }
        }
        ensure!(
            errors.is_empty(),
            "Unsupported MkDocs configuration:\n{}",
            errors.join("\n")
        );
        Ok(Self {
            title: text(map, "site_name")?.ok_or_else(|| {
                anyhow::anyhow!("mkdocs.yml requires site_name")
            })?,
            docs: text(map, "docs_dir")?.unwrap_or_else(|| "docs".into()),
            description: text(map, "site_description")?,
            repository: text(map, "repo_url")?,
            site_url: text(map, "site_url")?,
            nav: map.get("nav").cloned(),
            warnings,
        })
    }
}
fn text(map: &Mapping, key: &str) -> Result<Option<String>> {
    map.get(key)
        .map(|v| {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("{key} must be text"))?;
            ensure!(
                !s.trim().is_empty() && s.len() <= 4096,
                "{key} must contain 1–4096 bytes"
            );
            Ok(s.to_owned())
        })
        .transpose()
}
fn safe(value: &Value, depth: usize, count: &mut usize) -> Result<()> {
    *count += 1;
    ensure!(
        *count <= 4096 && depth <= 16,
        "MkDocs YAML exceeds 4096 nodes or depth 16"
    );
    match value {
        Value::Tagged(_) => {
            anyhow::bail!(
                "MkDocs YAML tags (including Python objects and !ENV) are not \
                 executed; use literal configuration values"
            )
        },
        Value::Sequence(values) => {
            for value in values {
                safe(value, depth + 1, count)?;
            }
        },
        Value::Mapping(values) => {
            for (key, value) in values {
                safe(key, depth + 1, count)?;
                safe(value, depth + 1, count)?;
            }
        },
        _ => {},
    }
    Ok(())
}
fn reject_tags(body: &str) -> Result<()> {
    for line in body.lines() {
        let mut quote = None;
        let mut escaped = false;
        let mut boundary = true;
        for ch in line.chars() {
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' && delimiter == '"' {
                    escaped = true;
                    continue;
                }
                if ch == delimiter {
                    quote = None;
                }
                continue;
            }
            if ch == '#' && boundary {
                break;
            }
            if ch == '\'' || ch == '"' {
                quote = Some(ch);
                continue;
            }
            ensure!(
                !(boundary && matches!(ch, '!' | '&' | '*')),
                "MkDocs YAML tags and aliases are not executed; replace \
                 Python tags, !ENV, anchors, and aliases with literal \
                 configuration values"
            );
            if !ch.is_whitespace() {
                boundary = matches!(ch, '[' | '{' | ',' | ':') ||
                    (boundary && ch == '-');
            }
        }
    }
    Ok(())
}
fn extensions(
    value: Option<&Value>,
    plugins: bool,
    errors: &mut Vec<String>,
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
            matches!(name, "search" | "awesome-pages")
        } else {
            matches!(
                name,
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
        if let Some(options) = options.filter(|v| !v.is_null()) {
            let map = options.as_mapping().ok_or_else(|| {
                anyhow::anyhow!("{name} options must be a mapping")
            })?;
            for (key, value) in map {
                let key = key.as_str().ok_or_else(|| {
                    anyhow::anyhow!("{name} option must be text")
                })?;
                let supported = match (name, key) {
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
