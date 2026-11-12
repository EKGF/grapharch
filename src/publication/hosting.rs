//! Assemble immutable website and specification artifacts for static hosting.
mod files;
mod routes;
use {
    anyhow::{Context, Result, ensure},
    serde::Deserialize,
    std::path::{Component, Path, PathBuf},
};

/// The static host serving a package; it alone determines the output layout.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Platform {
    /// A plain file tree, `<output-root>/static/<mount>`, uploaded as a Pages
    /// artifact. Pages serves files only: no routes or response headers.
    GithubPages,
    /// The Vercel Build Output API layout, `<output-root>/.vercel/output`,
    /// with routing in `config.json` beside `static/<mount>`.
    Vercel,
}

impl Platform {
    fn name(self) -> &'static str {
        match self {
            Self::GithubPages => "github-pages",
            Self::Vercel => "vercel",
        }
    }

    /// The directory holding the package, created atomically.
    fn package_parent(self, output_root: &Path) -> PathBuf {
        match self {
            Self::GithubPages => output_root.to_path_buf(),
            Self::Vercel => output_root.join(".vercel"),
        }
    }

    fn package_name(self) -> &'static str {
        match self {
            Self::GithubPages => "static",
            Self::Vercel => "output",
        }
    }

    /// The mounted file tree inside the package.
    fn mounted(self, package: &Path, mount: &str) -> PathBuf {
        match self {
            Self::GithubPages => package.join(mount),
            Self::Vercel => package.join("static").join(mount),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    /// Checked by [`Version`] before this schema is read.
    #[serde(rename = "version")]
    _version:          u8,
    platform:          Platform,
    mount:             String,
    site:              PathBuf,
    #[serde(default)]
    static_trees:      Vec<Tree>,
    #[serde(default)]
    directory_aliases: Vec<Alias>,
    #[serde(default)]
    file_aliases:      Vec<FileAlias>,
    inventory:         Option<PathBuf>,
    version_path:      Option<String>,
}
/// The version field alone, read before the versioned schema.
#[derive(Deserialize)]
struct Version {
    version: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    source:      PathBuf,
    destination: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Alias {
    path:   String,
    target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileAlias {
    path:         String,
    target:       String,
    content_type: String,
}

/// Assemble the static package of the configured hosting platform, without
/// modifying any input. Existing output is rejected rather than overwritten.
pub async fn package(
    config: &Path,
    output_root: &Path,
    mount: Option<&str>,
) -> Result<PathBuf> {
    let config = tokio::fs::canonicalize(config).await?;
    ensure!(
        tokio::fs::metadata(&config).await?.len() <= 256 * 1024,
        "hosting configuration exceeds 256 KiB"
    );
    let bytes = tokio::fs::read(&config).await?;
    let version: Version = serde_yaml::from_slice(&bytes)?;
    ensure!(
        version.version == 2,
        "unsupported hosting configuration version {}; version 2 requires an \
         explicit platform: github-pages or vercel",
        version.version
    );
    let mut settings: Configuration = serde_yaml::from_slice(&bytes)?;
    if let Some(mount) = mount {
        settings.mount = mount.into();
    }
    ensure!(
        settings.mount.starts_with('/'),
        "hosting mount must be an absolute URL path"
    );
    let mount = relative(settings.mount.trim_matches('/'))?;
    ensure!(
        settings.static_trees.len() <= 32 &&
            settings.directory_aliases.len() <= 32 &&
            settings.file_aliases.len() <= 64,
        "hosting mapping budget exceeded"
    );
    let parent = config.parent().expect("canonical file has parent");
    // bounded: at most 33 explicitly configured input artifact trees.
    let mut trees = vec![(
        tokio::fs::canonicalize(parent.join(&settings.site)).await?,
        String::new(),
    )];
    for tree in &settings.static_trees {
        trees.push((
            tokio::fs::canonicalize(parent.join(&tree.source)).await?,
            relative(&tree.destination)?,
        ));
    }
    let platform = settings.platform;
    tokio::fs::create_dir_all(platform.package_parent(output_root)).await?;
    let output_parent =
        tokio::fs::canonicalize(platform.package_parent(output_root)).await?;
    let final_path = output_parent.join(platform.package_name());
    ensure!(
        !tokio::fs::try_exists(&final_path).await?,
        "hosting output already exists: {}",
        final_path.display()
    );
    for (input, _) in &trees {
        ensure!(
            !output_parent.starts_with(input),
            "hosting output must not be inside an input artifact tree"
        );
    }
    let stage =
        output_parent.join(format!(".grapharch-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&stage).await?;
    let result = assemble(&settings, parent, &trees, &mount, &stage).await;
    if let Err(error) = result {
        tokio::fs::remove_dir_all(&stage)
            .await
            .context("remove failed hosting stage")?;
        return Err(error);
    }
    tokio::fs::rename(&stage, &final_path).await?;
    Ok(final_path)
}

async fn assemble(
    settings: &Configuration,
    parent: &Path,
    trees: &[(PathBuf, String)],
    mount: &str,
    stage: &Path,
) -> Result<()> {
    let root = settings.platform.mounted(stage, mount);
    tokio::fs::create_dir_all(&root).await?;
    let mut files = files::Inventory::new();
    for (source, destination) in trees {
        files.copy(source, &root, destination).await?;
    }
    for alias in &settings.directory_aliases {
        let path = relative(&alias.path)?;
        let target = relative(&alias.target)?;
        ensure!(
            !path.is_empty() && !target.is_empty(),
            "directory aliases must name nonempty paths"
        );
        let (source, prefix) = trees
            .iter()
            .filter(|(_, prefix)| {
                prefix.is_empty() ||
                    target == *prefix ||
                    target.starts_with(&format!("{prefix}/"))
            })
            .max_by_key(|(_, prefix)| prefix.len())
            .context("directory alias does not select an input tree")?;
        let suffix =
            target.strip_prefix(prefix).unwrap().trim_start_matches('/');
        files.copy(&source.join(suffix), &root, &path).await?;
    }
    ensure!(
        files.paths.contains("index.html"),
        "project artifact requires index.html"
    );
    let config =
        routes::configuration(settings, parent, mount, &files.paths).await?;
    match settings.platform {
        Platform::Vercel => {
            tokio::fs::write(
                stage.join("config.json"),
                serde_json::to_vec_pretty(&config)?,
            )
            .await?;
        },
        // Pages serves directory indexes, trailing-slash redirects and the
        // root 404.html itself; anything else needs a router it lacks.
        Platform::GithubPages => {
            config.require_files_only(settings.platform.name())?
        },
    }
    Ok(())
}

fn relative(value: &str) -> Result<String> {
    ensure!(
        !value.starts_with('/'),
        "publication path must be relative: {value}"
    );
    ensure!(
        value.is_empty() ||
            (Path::new(value)
                .components()
                .all(|part| matches!(part, Component::Normal(_))) &&
                value.split('/').all(|part| {
                    !part.is_empty() &&
                        part != "." &&
                        part != ".." &&
                        part.chars().all(|c| {
                            c.is_ascii_alphanumeric() ||
                                matches!(c, '-' | '_' | '.')
                        })
                })),
        "invalid publication path: {value}"
    );
    Ok(value.into())
}
