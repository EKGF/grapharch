//! `package.json`, including npm, Yarn and pnpm-style workspaces.
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
};

#[derive(Deserialize)]
#[serde(untagged)]
enum Repository {
    Url(String),
    Object { url: String },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Workspaces {
    Globs(Vec<String>),
    Object {
        #[serde(default)]
        packages: Vec<String>,
    },
}

#[derive(Deserialize)]
struct Manifest {
    name:        Option<String>,
    version:     Option<String>,
    description: Option<String>,
    license:     Option<String>,
    homepage:    Option<String>,
    repository:  Option<Repository>,
    #[serde(default)]
    keywords:    Vec<String>,
    #[serde(default)]
    private:     bool,
    workspaces:  Option<Workspaces>,
}

/// One parsed `package.json`.
pub(super) struct PackageJson {
    manifest:              Manifest,
    pub(super) workspaces: Vec<String>, // bounded: the manifest's globs.
}

impl PackageJson {
    pub(super) fn parse(text_: &str, path: &str) -> Result<Self> {
        let manifest: Manifest = serde_json::from_str(text_)
            .with_context(|| format!("parse {path}"))?;
        let workspaces = match &manifest.workspaces {
            Some(Workspaces::Globs(globs)) => globs.clone(),
            Some(Workspaces::Object { packages }) => packages.clone(),
            None => Vec::new(),
        };
        Ok(Self { manifest, workspaces })
    }

    pub(super) fn package(&self, path: &str) -> Result<Option<CodePackage>> {
        let manifest = &self.manifest;
        let Some(name) = &manifest.name else {
            return Ok(None)
        };
        let mut package = CodePackage::new(
            Ecosystem::Npm,
            PackageName::try_new(name)?,
            path.to_owned(),
        );
        package.version = manifest
            .version
            .as_deref()
            .map(PackageVersion::try_new)
            .transpose()?;
        package.description = text(manifest.description.as_deref());
        package.license = manifest
            .license
            .as_deref()
            .and_then(|value| SpdxExpression::try_new(value).ok());
        package.homepage = web_url(manifest.homepage.as_deref());
        package.repository = match &manifest.repository {
            Some(Repository::Url(url) | Repository::Object { url }) => {
                web_url(Some(url)).or_else(|| shorthand(url))
            },
            None => None,
        };
        package.keywords = words(manifest.keywords.iter().map(String::as_str));
        package.published = !manifest.private;
        Ok(Some(package))
    }
}

/// npm's `github:owner/repo` and `owner/repo` repository shorthands.
fn shorthand(value: &str) -> Option<url::Url> {
    let path = value.strip_prefix("github:").unwrap_or(value);
    let (owner, repo) = path.split_once('/')?;
    (!owner.contains(':') && !repo.contains('/'))
        .then(|| {
            url::Url::parse(&format!(
                "https://github.com/{owner}/{}",
                repo.trim_end_matches(".git")
            ))
            .ok()
        })
        .flatten()
}
