//! Typed facts of one package a manifest declares.
use {
    anyhow::{Result, ensure},
    url::Url,
};

/// The package ecosystem whose registry and conventions apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Ecosystem {
    Cargo,
    PyPI,
    Npm,
    Go,
}

impl Ecosystem {
    /// The language a package of this ecosystem is written in; npm also
    /// holds JavaScript, so TypeScript is detected separately.
    pub(crate) fn language(self) -> &'static str {
        match self {
            Self::Cargo => "Rust",
            Self::PyPI => "Python",
            Self::Npm => "JavaScript",
            Self::Go => "Go",
        }
    }

    /// Local name of the ecosystem's individual in the GraphArch vocabulary.
    pub(crate) fn term(self) -> &'static str {
        match self {
            Self::Cargo => "CargoEcosystem",
            Self::PyPI => "PyPIEcosystem",
            Self::Npm => "NpmEcosystem",
            Self::Go => "GoEcosystem",
        }
    }
}

/// A package name as its registry spells it (`@scope/name` on npm, a module
/// path for Go).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageName(String);

impl PackageName {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() &&
                value.len() <= 214 &&
                value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"-_.@/~".contains(&byte)
                }) &&
                !value.contains(".."),
            "invalid package name {value:?}"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

/// A declared package version. Semantic versions (Cargo, npm) parse as
/// [`semver::Version`]; PEP 440 versions are kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackageVersion(String);

impl PackageVersion {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() &&
                value.len() <= 64 &&
                value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b".-+!_".contains(&byte)
                }),
            "invalid package version {value:?}"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }

    #[cfg(test)]
    pub(crate) fn semver(&self) -> Option<semver::Version> {
        semver::Version::parse(self.0.trim_start_matches('v')).ok()
    }
}

/// An SPDX license expression, such as `MIT OR Apache-2.0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpdxExpression(String);

impl SpdxExpression {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        let value = value.trim();
        ensure!(
            !value.is_empty() &&
                value.len() <= 256 &&
                value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b" .-+()/:".contains(&byte)
                }),
            "invalid SPDX license expression {value:?}"
        );
        Ok(Self(
            value.split_whitespace().collect::<Vec<_>>().join(" "),
        ))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }

    /// The SPDX license IRI when the expression is one license identifier.
    pub(crate) fn license_iri(&self) -> Option<Url> {
        let id = self.0.as_str();
        (!id.contains(' ') && !id.contains('(') && !id.contains('/'))
            .then(|| Url::parse(&format!("http://spdx.org/licenses/{id}")).ok())
            .flatten()
    }
}

/// One package declared by a manifest in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CodePackage {
    pub(crate) ecosystem:     Ecosystem,
    pub(crate) name:          PackageName,
    pub(crate) version:       Option<PackageVersion>,
    pub(crate) description:   Option<String>,
    pub(crate) license:       Option<SpdxExpression>,
    pub(crate) repository:    Option<Url>,
    pub(crate) homepage:      Option<Url>,
    pub(crate) documentation: Option<Url>,
    pub(crate) keywords:      Vec<String>, // bounded: MAX_KEYWORDS.
    pub(crate) categories:    Vec<String>, // bounded: MAX_KEYWORDS.
    /// Slash-separated manifest path relative to the repository root.
    pub(crate) manifest:      String,
    /// Whether the package may be published to its registry.
    pub(crate) published:     bool,
}

/// Most keywords or categories retained per package.
pub(crate) const MAX_KEYWORDS: usize = 32;

impl CodePackage {
    pub(crate) fn new(
        ecosystem: Ecosystem,
        name: PackageName,
        manifest: String,
    ) -> Self {
        Self {
            ecosystem,
            name,
            version: None,
            description: None,
            license: None,
            repository: None,
            homepage: None,
            documentation: None,
            keywords: Vec::new(),
            categories: Vec::new(),
            manifest,
            published: true,
        }
    }

    /// The package's registry page, when it is published to one.
    pub(crate) fn registry_page(&self) -> Option<Url> {
        if !self.published {
            return None;
        }
        let name = self.name.as_str();
        Url::parse(&match self.ecosystem {
            Ecosystem::Cargo => format!("https://crates.io/crates/{name}"),
            Ecosystem::PyPI => format!("https://pypi.org/project/{name}/"),
            Ecosystem::Npm => format!("https://www.npmjs.com/package/{name}"),
            Ecosystem::Go => format!("https://pkg.go.dev/{name}"),
        })
        .ok()
    }

    /// The package's API reference: declared, else docs.rs for a crate.
    pub(crate) fn api_reference(&self) -> Option<Url> {
        self.documentation.clone().or_else(|| {
            (self.published && self.ecosystem == Ecosystem::Cargo)
                .then(|| {
                    Url::parse(&format!(
                        "https://docs.rs/{}",
                        self.name.as_str()
                    ))
                    .ok()
                })
                .flatten()
        })
    }
}

/// A bounded, trimmed text field from a manifest; empty values are absent.
pub(crate) fn text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(1024).collect())
}

/// An HTTP(S) URL from a manifest; anything else is not a link.
pub(crate) fn web_url(value: Option<&str>) -> Option<Url> {
    let url = Url::parse(value?.trim().trim_start_matches("git+")).ok()?;
    (matches!(url.scheme(), "https" | "http") && url.host_str().is_some())
        .then_some(url)
}

/// Bounded keyword list.
pub(crate) fn words<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    values
        .filter_map(|value| text(Some(value)))
        .take(MAX_KEYWORDS)
        .collect()
}
