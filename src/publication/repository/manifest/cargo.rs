//! `Cargo.toml`, including workspace members and inherited fields.
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

/// A field written directly or as `field.workspace = true`.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Inheritable<T> {
    Value(T),
    Workspace { workspace: bool },
}

impl<T: Clone> Inheritable<T> {
    fn resolve(&self, inherited: Option<&T>) -> Option<T> {
        match self {
            Self::Value(value) => Some(value.clone()),
            Self::Workspace { workspace: true } => inherited.cloned(),
            Self::Workspace { .. } => None,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct Fields {
    version:       Option<Inheritable<String>>,
    description:   Option<Inheritable<String>>,
    license:       Option<Inheritable<String>>,
    repository:    Option<Inheritable<String>>,
    homepage:      Option<Inheritable<String>>,
    documentation: Option<Inheritable<String>>,
    keywords:      Option<Inheritable<Vec<String>>>,
    categories:    Option<Inheritable<Vec<String>>>,
    publish:       Option<Inheritable<Publish>>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Publish {
    Flag(bool),
    Registries(Vec<String>),
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "kebab-case")]
struct WorkspaceFields {
    version:       Option<String>,
    description:   Option<String>,
    license:       Option<String>,
    repository:    Option<String>,
    homepage:      Option<String>,
    documentation: Option<String>,
    keywords:      Option<Vec<String>>,
    categories:    Option<Vec<String>>,
    publish:       Option<Publish>,
}

#[derive(Deserialize)]
struct Package {
    name:   String,
    #[serde(flatten)]
    fields: Fields,
}

#[derive(Deserialize, Default, Clone)]
pub(super) struct Workspace {
    #[serde(default)]
    pub(super) members: Vec<String>,
    #[serde(default)]
    pub(super) exclude: Vec<String>,
    #[serde(default)]
    package:            WorkspaceFields,
}

#[derive(Deserialize)]
struct Manifest {
    package:   Option<Package>,
    workspace: Option<Workspace>,
}

/// One parsed `Cargo.toml`.
pub(super) struct CargoManifest {
    package:              Option<Package>,
    pub(super) workspace: Option<Workspace>,
}

impl CargoManifest {
    pub(super) fn parse(text: &str, path: &str) -> Result<Self> {
        let manifest: Manifest =
            toml::from_str(text).with_context(|| format!("parse {path}"))?;
        Ok(Self {
            package:   manifest.package,
            workspace: manifest.workspace,
        })
    }

    /// The declared package, with workspace-inherited fields resolved
    /// against the root's `[workspace.package]`.
    pub(super) fn package(
        &self,
        manifest: &str,
        root: Option<&Workspace>,
    ) -> Result<Option<CodePackage>> {
        let Some(package) = &self.package else {
            return Ok(None)
        };
        let inherited = root
            .or(self.workspace.as_ref())
            .map(|workspace| workspace.package.clone())
            .unwrap_or_default();
        let fields = &package.fields;
        let string = |field: &Option<Inheritable<String>>,
                      inherited: &Option<String>| {
            field
                .as_ref()
                .and_then(|field| field.resolve(inherited.as_ref()))
        };
        let mut result = CodePackage::new(
            Ecosystem::Cargo,
            PackageName::try_new(&package.name)?,
            manifest.to_owned(),
        );
        result.version = string(&fields.version, &inherited.version)
            .map(|value| PackageVersion::try_new(&value))
            .transpose()?;
        result.description = text(
            string(&fields.description, &inherited.description).as_deref(),
        );
        result.license = string(&fields.license, &inherited.license)
            .map(|value| SpdxExpression::try_new(&value))
            .transpose()?;
        result.repository = web_url(
            string(&fields.repository, &inherited.repository).as_deref(),
        );
        result.homepage =
            web_url(string(&fields.homepage, &inherited.homepage).as_deref());
        result.documentation = web_url(
            string(&fields.documentation, &inherited.documentation).as_deref(),
        );
        let list = |field: &Option<Inheritable<Vec<String>>>,
                    inherited: &Option<Vec<String>>| {
            field
                .as_ref()
                .and_then(|field| field.resolve(inherited.as_ref()))
                .unwrap_or_default()
        };
        result.keywords = words(
            list(&fields.keywords, &inherited.keywords)
                .iter()
                .map(String::as_str),
        );
        result.categories = words(
            list(&fields.categories, &inherited.categories)
                .iter()
                .map(String::as_str),
        );
        result.published = !matches!(
            fields
                .publish
                .as_ref()
                .and_then(|field| field.resolve(inherited.publish.as_ref())),
            Some(Publish::Flag(false))
        ) && !matches!(
            fields
                .publish
                .as_ref()
                .and_then(|field| field.resolve(inherited.publish.as_ref())),
            Some(Publish::Registries(registries)) if registries.is_empty()
        );
        Ok(Some(result))
    }
}
