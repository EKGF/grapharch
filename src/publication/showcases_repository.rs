//! Build a zero-configuration documentation website from selected paths of a
//! pinned, non-executing Git tree.
use {
    super::{
        collect::{Collector, TextAssetFile, git},
        dataset::{Dataset, identity},
        plan::ResolvedPlan,
        quick_build::pinned,
        showcase_configuration::ShowcaseConfiguration,
    },
    anyhow::{Result, ensure},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// The GraphArch settings of a repository showcase.
const SETTINGS: &[&str] = &[
    "gitRepository",
    "gitCommit",
    "materializedPath",
    "licensePath",
];

/// The pinned repository paths a showcase publishes; every license is
/// retained as a downloadable asset.
struct Source {
    repository: String,
    commit:     String,
    paths:      Vec<PathBuf>,
    licenses:   Vec<PathBuf>,
}

impl Source {
    fn read(configuration: &ShowcaseConfiguration) -> Result<Self> {
        configuration.accept_only("repository", SETTINGS)?;
        let paths = configuration
            .texts("materializedPath")?
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        ensure!(
            !paths.is_empty(),
            "repository showcase {} requires ga:materializedPath",
            configuration.id()
        );
        let commit = configuration.text("gitCommit")?;
        ensure!(
            gix::hash::ObjectId::from_hex(commit.as_bytes()).is_ok(),
            "repository showcase {} requires an immutable Git commit",
            configuration.id()
        );
        let licenses = configuration
            .texts("licensePath")?
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        ensure!(
            !licenses.is_empty() &&
                licenses.iter().all(|license| {
                    paths.iter().any(|path| license.starts_with(path))
                }),
            "repository showcase {} must materialize every ga:licensePath",
            configuration.id()
        );
        Ok(Self {
            repository: configuration.text("gitRepository")?,
            commit,
            paths,
            licenses,
        })
    }

    /// Materialize the selected paths and collect them with the
    /// zero-configuration defaults, retaining the license and Git provenance.
    async fn collect(&self, cache: &Path) -> Result<(Dataset, ResolvedPlan)> {
        let source =
            git::documentation_source(&self.repository, &self.licenses[0])
                .await?;
        let project = identity("quick-project", &[source.as_str()]);
        let repository = git::remote_documentation(
            &self.repository,
            &self.commit,
            cache,
            project.as_str(),
        )
        .await?;
        ensure!(
            repository.commit == self.commit,
            "resolved repository commit differs from the selected revision"
        );
        let tree = tempfile::tempdir()?;
        repository
            .materialize_documentation(&self.paths, tree.path())
            .await?;
        let root = tokio::fs::canonicalize(tree.path()).await?;
        let mut extra: Vec<Arc<dyn Collector>> =
            vec![repository.provenance.clone()]; // bounded: one license asset and provenance record per selected license.
        for path in &self.licenses {
            let blob = repository.blob(path).await?;
            let license = TextAssetFile::new(
                project.as_str(),
                identity("upstream-license", &[
                    project.as_str(),
                    path.to_str().unwrap_or_default(),
                ])
                .as_str(),
                root.clone(),
                root.join(path),
            )?
            .with_source_identity(blob.source())?;
            extra.push(Arc::new(license));
            extra.push(blob.provenance_collector());
        }
        pinned::collect(&root, &source, extra).await
    }
}

/// Collect a pinned public repository without executing any of its files.
pub async fn build(
    configuration: &Path,
    cache: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    configured(
        &ShowcaseConfiguration::read(configuration).await?,
        cache,
    )
    .await
}

pub(super) async fn configured(
    configuration: &ShowcaseConfiguration,
    cache: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let (dataset, plan) = Source::read(configuration)?.collect(cache).await?;
    Ok((dataset, vec![plan]))
}
