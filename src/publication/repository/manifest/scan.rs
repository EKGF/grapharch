//! Find the manifests at a repository root and the workspace members they
//! name.
use {
    super::{
        cargo::CargoManifest,
        go_mod,
        package_json::PackageJson,
        pyproject,
    },
    crate::publication::repository::package::CodePackage,
    anyhow::{Context, Result, ensure},
    globset::{GlobBuilder, GlobSet, GlobSetBuilder},
    std::path::{Path, PathBuf},
};

/// Largest manifest read.
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
/// Most workspace members examined per workspace.
const MAX_MEMBERS: usize = 1024;

/// The packages the manifests of one repository declare, root first.
#[derive(Clone, Debug, Default)]
pub(crate) struct Manifests {
    packages: Vec<CodePackage>, // bounded: MAX_MEMBERS per ecosystem.
}

impl Manifests {
    pub(crate) async fn read(root: &Path) -> Result<Self> {
        let mut packages = Vec::new(); // bounded: MAX_MEMBERS per ecosystem.
        if let Some(text) = read(root, "Cargo.toml").await? {
            let manifest = CargoManifest::parse(&text, "Cargo.toml")?;
            packages.extend(manifest.package("Cargo.toml", None)?);
            if let Some(workspace) = &manifest.workspace {
                for member in
                    members(root, &workspace.members, &workspace.exclude)
                        .await?
                {
                    let path = format!("{member}/Cargo.toml");
                    let Some(text) = read(root, &path).await? else {
                        continue
                    };
                    packages.extend(
                        CargoManifest::parse(&text, &path)?
                            .package(&path, Some(workspace))?,
                    );
                }
            }
        }
        if let Some(text) = read(root, "pyproject.toml").await? {
            packages.extend(pyproject::package(&text, "pyproject.toml")?);
        }
        if let Some(text) = read(root, "package.json").await? {
            let manifest = PackageJson::parse(&text, "package.json")?;
            packages.extend(manifest.package("package.json")?);
            let mut globs = manifest.workspaces.clone();
            if let Some(pnpm) = read(root, "pnpm-workspace.yaml").await? {
                globs.extend(pnpm_packages(&pnpm)?);
            }
            for member in members(root, &globs, &[]).await? {
                let path = format!("{member}/package.json");
                let Some(text) = read(root, &path).await? else {
                    continue
                };
                packages
                    .extend(PackageJson::parse(&text, &path)?.package(&path)?);
            }
        }
        if let Some(text) = read(root, "go.mod").await? {
            packages.extend(go_mod::package(&text, "go.mod")?);
        }
        Ok(Self { packages })
    }

    pub(crate) fn packages(&self) -> &[CodePackage] { &self.packages }

    /// The package that names the repository: the root manifest's package,
    /// else the only workspace member.
    pub(crate) fn primary(&self) -> Option<&CodePackage> {
        self.packages
            .iter()
            .find(|package| !package.manifest.contains('/'))
            .or_else(|| (self.packages.len() == 1).then(|| &self.packages[0]))
    }

    /// A field shared by every package, such as a workspace-wide version.
    pub(crate) fn common<T: PartialEq + Clone>(
        &self,
        field: impl Fn(&CodePackage) -> Option<T>,
    ) -> Option<T> {
        let mut values = self.packages.iter().map(&field);
        let first = values.next()??;
        values
            .all(|value| value.as_ref() == Some(&first))
            .then_some(first)
    }
}

/// The manifest at `relative`, or `None` when it does not exist.
async fn read(root: &Path, relative: &str) -> Result<Option<String>> {
    let path = root.join(relative);
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        },
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    ensure!(
        metadata.len() <= MAX_MANIFEST_BYTES,
        "{relative} exceeds {MAX_MANIFEST_BYTES} bytes"
    );
    Ok(Some(
        tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read {relative}"))?,
    ))
}

/// pnpm's `packages:` list.
fn pnpm_packages(text: &str) -> Result<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct Workspace {
        #[serde(default)]
        packages: Vec<String>,
    }
    let workspace: Workspace =
        serde_yaml::from_str(text).context("parse pnpm-workspace.yaml")?;
    Ok(workspace.packages)
}

fn globs(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
        ensure!(
            !pattern.starts_with('/') && !pattern.split('/').any(|p| p == ".."),
            "workspace member {pattern} must stay inside the repository"
        );
        builder.add(GlobBuilder::new(pattern).literal_separator(true).build()?);
    }
    Ok(builder.build()?)
}

/// Directories matching the workspace's member globs, sorted.
async fn members(
    root: &Path,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<String>> {
    let (positive, negative): (Vec<String>, Vec<String>) = include
        .iter()
        .cloned()
        .partition(|glob| !glob.starts_with('!'));
    let negative: Vec<String> = negative
        .iter()
        .map(|glob| glob.trim_start_matches('!').to_owned())
        .chain(exclude.iter().cloned())
        .collect();
    if positive.is_empty() {
        return Ok(Vec::new());
    }
    let (include, exclude) = (globs(&positive)?, globs(&negative)?);
    let depth = positive
        .iter()
        .map(|glob| {
            if glob.contains("**") {
                8
            } else {
                glob.trim_matches('/').split('/').count()
            }
        })
        .max()
        .unwrap_or(1);
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new(); // bounded: MAX_MEMBERS.
        let walker = ignore::WalkBuilder::new(&root)
            .max_depth(Some(depth))
            .require_git(false)
            .follow_links(false)
            .sort_by_file_path(Ord::cmp)
            .filter_entry(|entry| {
                entry.file_name() != "node_modules" &&
                    entry.file_name() != "target"
            })
            .build();
        for entry in walker {
            let entry = entry?;
            if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                continue;
            }
            let relative: PathBuf =
                entry.path().strip_prefix(&root)?.to_owned();
            let Some(relative) = relative.to_str() else {
                continue
            };
            let relative = relative.replace('\\', "/");
            if relative.is_empty() ||
                !include.is_match(&relative) ||
                exclude.is_match(&relative)
            {
                continue;
            }
            ensure!(
                found.len() < MAX_MEMBERS,
                "workspace exceeds {MAX_MEMBERS} members"
            );
            found.push(relative);
        }
        Ok(found)
    })
    .await?
}
