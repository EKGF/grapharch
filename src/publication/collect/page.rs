//! Normalized page hints for one Markdown source, independent of rendering.
use {
    anyhow::{Result, ensure},
    serde::Deserialize,
    std::path::{Component, Path},
};

#[derive(Default, Deserialize)]
pub(super) struct PageHints {
    pub permalink:   Option<String>,
    pub parent:      Option<String>,
    pub nav_order:   Option<u32>,
    #[serde(default)]
    pub nav_exclude: bool,
}

pub(crate) fn source_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    ensure!(
        relative
            .components()
            .all(|p| matches!(p, Component::Normal(_))),
        "source escapes documentation root"
    );
    Ok(relative
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("source path must be UTF-8"))?
        .replace('\\', "/"))
}

pub(crate) fn route(source: &str, permalink: Option<&str>) -> Result<String> {
    let value = if let Some(permalink) = permalink {
        permalink.trim_matches('/').to_owned()
    } else if source == "index.md" || source == "README.md" {
        String::new()
    } else if let Some(directory) = source
        .strip_suffix("/README.md")
        .or_else(|| source.strip_suffix("/index.md"))
    {
        directory.to_owned()
    } else {
        source
            .strip_suffix(".md")
            .ok_or_else(|| anyhow::anyhow!("page must be Markdown"))?
            .to_owned()
    };
    ensure!(
        value.is_empty() ||
            value.split('/').all(|part| {
                !part.is_empty() &&
                    part != "." &&
                    part != ".." &&
                    part.chars().all(|c| {
                        c.is_ascii_alphanumeric() ||
                            matches!(c, '-' | '_' | '.')
                    })
            }),
        "invalid publication route {value}"
    );
    Ok(if value.is_empty() {
        value
    } else {
        format!("{value}/")
    })
}

pub(crate) fn slug(text: &str) -> String {
    let mut output = String::new();
    for character in text.to_lowercase().chars() {
        if character.is_alphanumeric() || character == '_' {
            output.push(character);
        } else if (character.is_whitespace() || character == '-') &&
            !output.ends_with('-')
        {
            output.push('-');
        }
    }
    let value = output.trim_matches('-');
    if value.is_empty() {
        "heading".into()
    } else {
        value.into()
    }
}
