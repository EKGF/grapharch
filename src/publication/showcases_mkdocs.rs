//! Build a source-owned MkDocs publication from a pinned, non-executing Git
//! tree.
use {
    super::{
        collect::{Collector, TextAssetFile, git, mkdocs::MkDocs},
        dataset::{Dataset, identity},
        plan::ResolvedPlan,
        showcase_configuration::ShowcaseConfiguration,
        website,
    },
    anyhow::{Result, ensure},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// The GraphArch settings of a MkDocs showcase.
const SETTINGS: &[&str] = &[
    "gitRepository",
    "gitCommit",
    "mkdocsConfiguration",
    "materializedPath",
    "licensePath",
];

/// The pinned MkDocs tree a showcase imports.
struct Source {
    repository:    String,
    commit:        String,
    configuration: PathBuf,
    paths:         Vec<PathBuf>,
    license:       PathBuf,
}

impl Source {
    fn read(configuration: &ShowcaseConfiguration) -> Result<Self> {
        configuration.accept_only("MkDocs", SETTINGS)?;
        let paths = configuration
            .texts("materializedPath")?
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        ensure!(
            !paths.is_empty(),
            "MkDocs showcase {} requires ga:materializedPath",
            configuration.id()
        );
        Ok(Self {
            repository: configuration.text("gitRepository")?,
            commit: configuration.text("gitCommit")?,
            configuration: configuration.text("mkdocsConfiguration")?.into(),
            paths,
            license: configuration.text("licensePath")?.into(),
        })
    }
}

/// Collect a pinned public documentation example without running MkDocs or
/// Python.
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
    let source = Source::read(configuration)?;
    ensure!(
        gix::hash::ObjectId::from_hex(source.commit.as_bytes()).is_ok(),
        "MkDocs showcase requires an immutable Git commit"
    );
    let source_id =
        git::documentation_source(&source.repository, &source.configuration)
            .await?;
    let project = MkDocs::project_for_source(&source_id)?;
    let repository = Arc::new(
        git::remote_documentation(
            &source.repository,
            &source.commit,
            cache,
            project.as_str(),
        )
        .await?,
    );
    ensure!(
        repository.commit == source.commit,
        "resolved documentation commit differs from the selected revision"
    );
    let tree = tempfile::tempdir()?;
    repository
        .materialize_documentation(&source.paths, tree.path())
        .await?;
    let root = tokio::fs::canonicalize(tree.path()).await?;
    let config_path = root.join(&source.configuration);
    let importer = MkDocs::new(&root, &config_path)
        .await?
        .with_source_identity(&source_id)?;
    let target = importer.target.clone();
    let license_blob = repository.blob(&source.license).await?;
    let configuration_blob = repository.blob(&source.configuration).await?;
    let license = TextAssetFile::new(
        project.as_str(),
        identity("upstream-license", &[project.as_str()]).as_str(),
        root.clone(),
        root.join(&source.license),
    )?
    .with_source_identity(license_blob.source())?;
    let extra_collectors: Vec<Arc<dyn Collector>> = vec![
        Arc::new(license),
        license_blob.provenance_collector(),
        configuration_blob.provenance_collector(),
    ];
    let publication = website::collect_with_configuration(
        &root,
        &config_path,
        project.as_str(),
        target.as_str(),
        4,
        &super::collect::default_http_image_factory,
        website::SourceConfiguration {
            collector: Arc::new(importer),
            mkdocs_metadata: true,
            repository: Some(repository),
            extra_collectors,
        },
    )
    .await?;
    Ok((publication.dataset, vec![publication.plan]))
}
