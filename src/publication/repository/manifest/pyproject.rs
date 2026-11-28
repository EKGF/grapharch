//! `pyproject.toml` `[project]` metadata (PEP 621), with Poetry's table as
//! the fallback.
use {
    super::super::package::{
        CodePackage,
        Ecosystem,
        PackageName,
        PackageVersion,
        SpdxExpression,
        text,
        web_url,
        words,
    },
    anyhow::{Context, Result},
    serde::Deserialize,
    std::collections::BTreeMap,
};

#[derive(Deserialize)]
#[serde(untagged)]
enum License {
    Expression(String),
    Table { text: Option<String> },
}

#[derive(Deserialize)]
struct Project {
    name:        String,
    version:     Option<String>,
    description: Option<String>,
    license:     Option<License>,
    #[serde(default)]
    keywords:    Vec<String>,
    #[serde(default)]
    classifiers: Vec<String>,
    #[serde(default)]
    urls:        BTreeMap<String, String>, /* bounded: the manifest's URL
                                            * table. */
}

#[derive(Deserialize)]
struct Poetry {
    name:          Option<String>,
    version:       Option<String>,
    description:   Option<String>,
    license:       Option<String>,
    repository:    Option<String>,
    homepage:      Option<String>,
    documentation: Option<String>,
    #[serde(default)]
    keywords:      Vec<String>,
}

#[derive(Deserialize)]
struct Tool {
    poetry: Option<Poetry>,
}

#[derive(Deserialize)]
struct Manifest {
    project: Option<Project>,
    tool:    Option<Tool>,
}

pub(super) fn package(
    text_: &str,
    manifest: &str,
) -> Result<Option<CodePackage>> {
    let parsed: Manifest =
        toml::from_str(text_).with_context(|| format!("parse {manifest}"))?;
    if let Some(project) = parsed.project {
        let mut package = CodePackage::new(
            Ecosystem::PyPI,
            PackageName::try_new(&project.name)?,
            manifest.to_owned(),
        );
        package.version = project
            .version
            .as_deref()
            .map(PackageVersion::try_new)
            .transpose()?;
        package.description = text(project.description.as_deref());
        package.license = match project.license {
            Some(License::Expression(value)) => {
                SpdxExpression::try_new(&value).ok()
            },
            Some(License::Table { text: Some(value) }) => {
                SpdxExpression::try_new(&value).ok()
            },
            _ => None,
        };
        let url = |names: &[&str]| {
            project.urls.iter().find_map(|(key, value)| {
                names
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name))
                    .then(|| web_url(Some(value)))
                    .flatten()
            })
        };
        package.repository =
            url(&["repository", "source", "source code", "code"]);
        package.homepage = url(&["homepage", "home"]);
        package.documentation = url(&["documentation", "docs"]);
        package.keywords = words(project.keywords.iter().map(String::as_str));
        package.categories = words(
            project
                .classifiers
                .iter()
                .filter_map(|value| value.strip_prefix("Topic :: ")),
        );
        return Ok(Some(package));
    }
    let Some(poetry) = parsed.tool.and_then(|tool| tool.poetry) else {
        return Ok(None);
    };
    let Some(name) = poetry.name else {
        return Ok(None)
    };
    let mut package = CodePackage::new(
        Ecosystem::PyPI,
        PackageName::try_new(&name)?,
        manifest.to_owned(),
    );
    package.version = poetry
        .version
        .as_deref()
        .map(PackageVersion::try_new)
        .transpose()?;
    package.description = text(poetry.description.as_deref());
    package.license = poetry
        .license
        .as_deref()
        .and_then(|v| SpdxExpression::try_new(v).ok());
    package.repository = web_url(poetry.repository.as_deref());
    package.homepage = web_url(poetry.homepage.as_deref());
    package.documentation = web_url(poetry.documentation.as_deref());
    package.keywords = words(poetry.keywords.iter().map(String::as_str));
    Ok(Some(package))
}
