//! Map Material for MkDocs plugins to GraphArch capabilities. A plugin is
//! never executed: its options are data that either configure a native
//! feature or yield a typed diagnostic.
use {
    super::{extensions::Hints, warning::ImportWarning},
    anyhow::{Result, ensure},
    serde_yaml::Value,
};

/// A recognized MkDocs plugin name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Plugin {
    Search,
    AwesomePages,
    IncludeMarkdown,
    MaterialEkgf,
    GitRevisionDate,
    Exclude,
    ExcludeSearch,
    BuildPlantuml,
    Kroki,
    GenFiles,
    Redirects,
    Social,
    Tags,
}
impl Plugin {
    pub(super) fn named(name: &str) -> Option<Self> {
        Some(match name {
            "search" | "material/search" => Self::Search,
            "awesome-pages" => Self::AwesomePages,
            "include-markdown" => Self::IncludeMarkdown,
            "material-ekgf" => Self::MaterialEkgf,
            "git-revision-date-localized" => Self::GitRevisionDate,
            "exclude" => Self::Exclude,
            "exclude-search" => Self::ExcludeSearch,
            "build_plantuml" => Self::BuildPlantuml,
            "kroki" => Self::Kroki,
            "gen-files" => Self::GenFiles,
            "redirects" => Self::Redirects,
            "social" | "material/social" => Self::Social,
            "tags" | "material/tags" => Self::Tags,
            _ => return None,
        })
    }

    /// Read this plugin's options into native hints or diagnostics.
    pub(super) fn apply(
        self,
        name: &str,
        options: Option<&Value>,
        hints: &mut Hints,
        warnings: &mut Vec<ImportWarning>,
    ) -> Result<()> {
        let options = options.filter(|v| !v.is_null());
        match self {
            Self::Search => {
                if options.is_some() {
                    warnings.push(ImportWarning::native(format!(
                        "{name} options (lang, separator): GraphArch's \
                         built-in search indexes every published page"
                    )));
                }
            },
            Self::AwesomePages => awesome_pages(options, warnings)?,
            Self::IncludeMarkdown => {},
            Self::MaterialEkgf => {
                warnings.push(ImportWarning::native(format!(
                    "{name}: presented with GraphArch's EKGF documentation \
                     theme; plugin options such as social cards are not \
                     reproduced"
                )))
            },
            Self::GitRevisionDate => {
                warnings.push(ImportWarning::native(format!(
                    "{name}: page dates come from GraphArch's Git provenance; \
                     locale, timezone and timeago display options are not \
                     reproduced"
                )))
            },
            Self::Exclude => exclude(options, hints, warnings)?,
            Self::ExcludeSearch => {
                warnings.push(ImportWarning::native(format!(
                    "{name}: pages excluded from navigation are already \
                     absent from GraphArch search; other search exclusions \
                     are not reproduced"
                )))
            },
            Self::BuildPlantuml => {
                hints.plantuml =
                    Some(super::plantuml_hint::PlantUmlHint::read(options)?);
            },
            Self::Kroki => {
                warnings.push(ImportWarning::not_executed(format!(
                    "{name}: diagrams are never sent to a remote rendering \
                     server; kroki fences are kept as source"
                )))
            },
            Self::GenFiles => {
                warnings.push(ImportWarning::not_executed(format!(
                    "{name}: generator scripts are never executed; generated \
                     pages need a native GraphArch documentor"
                )))
            },
            Self::Redirects => redirects(name, options, warnings)?,
            Self::Social | Self::Tags => {
                warnings.push(ImportWarning::substituted(format!(
                    "{name}: not reproduced by GraphArch"
                )))
            },
        }
        Ok(())
    }
}

fn awesome_pages(
    options: Option<&Value>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<()> {
    let Some(options) = options else {
        return Ok(())
    };
    let map = options.as_mapping().ok_or_else(|| {
        anyhow::anyhow!("awesome-pages options must be a mapping")
    })?;
    for (key, value) in map {
        match (key.as_str(), value) {
            // Both `.pages` and `.pages.yaml` are always read.
            (Some("filename"), Value::String(file))
                if matches!(file.as_str(), ".pages" | ".pages.yaml") => {},
            // GraphArch keeps single-page sections and rejects broken entries.
            (Some("collapse_single_pages"), Value::Bool(false)) |
            (Some("strict"), Value::Bool(true)) => {},
            (Some(key), _) => {
                warnings.push(ImportWarning::substituted(format!(
                    "awesome-pages.{key}: retained as source metadata; \
                     GraphArch reads .pages and .pages.yaml strictly without \
                     collapsing sections"
                )))
            },
            (None, _) => {
                anyhow::bail!("awesome-pages option names must be text")
            },
        }
    }
    Ok(())
}

fn exclude(
    options: Option<&Value>,
    hints: &mut Hints,
    warnings: &mut Vec<ImportWarning>,
) -> Result<()> {
    let Some(map) = options.and_then(Value::as_mapping) else {
        return Ok(())
    };
    if let Some(globs) = map.get("glob") {
        let globs = globs
            .as_sequence()
            .ok_or_else(|| anyhow::anyhow!("exclude.glob must be a list"))?;
        ensure!(
            globs.len() <= 64,
            "exclude.glob allows at most 64 patterns"
        );
        for glob in globs {
            let glob = glob.as_str().ok_or_else(|| {
                anyhow::anyhow!("exclude.glob entries must be text")
            })?;
            hints.exclude_patterns.push(glob.to_owned());
        }
    }
    if map.contains_key("regex") {
        warnings.push(ImportWarning::substituted(
            "exclude.regex: regular expressions are not translated; files \
             starting with '_' and non-Markdown files are not published as \
             pages",
        ));
    }
    Ok(())
}

fn redirects(
    name: &str,
    options: Option<&Value>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<()> {
    let maps = options
        .and_then(|o| o.get("redirect_maps"))
        .and_then(Value::as_mapping);
    for (from, to) in maps.into_iter().flatten() {
        let (Some(from), Some(to)) = (from.as_str(), to.as_str()) else {
            anyhow::bail!("{name}.redirect_maps entries must be text");
        };
        warnings.push(ImportWarning::substituted(format!(
            "{name}: {from} -> {to} is not published; declare a route alias \
             on the target page"
        )));
    }
    Ok(())
}
