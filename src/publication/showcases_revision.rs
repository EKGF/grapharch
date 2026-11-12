//! A gallery showcase built exactly like `publication build-revision`: the
//! definitions come from an exact controlling commit and only content from an
//! exact designated commit, both fetched through one bare transport cache.
use {
    super::{
        collect::git::{self, CommittedDefinitions},
        dataset::Dataset,
        plan::ResolvedPlan,
        revision_build::{
            self,
            PublicationUrl,
            RevisionInputs,
            TargetSelection,
        },
        showcase_configuration::ShowcaseConfiguration,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::NamedNode,
    std::path::{Path, PathBuf},
};

/// The GraphArch settings of an exact-revision showcase.
const SETTINGS: &[&str] = &[
    "gitRepository",
    "gitCommit",
    "definitionsCommit",
    "definitionsPath",
    "showcaseProject",
    "showcaseTargets",
    "allTargets",
];

/// `ga:allTargets true`, or an ordered `ga:showcaseTargets` list.
fn selection(configuration: &ShowcaseConfiguration) -> Result<TargetSelection> {
    match (
        configuration.flag("allTargets")?,
        configuration.node_list("showcaseTargets")?,
    ) {
        (Some(true), None) => Ok(TargetSelection::All),
        (None, Some(targets)) => TargetSelection::targets(targets),
        _ => {
            bail!(
                "showcase {} requires either ga:allTargets true or an ordered \
                 ga:showcaseTargets list",
                configuration.id()
            )
        },
    }
}

/// Collect and plan one exact-revision showcase. Mutable references are
/// rejected before any network access or cache creation.
pub(super) async fn build(
    configuration: &ShowcaseConfiguration,
    cache: &Path,
    publication_url: PublicationUrl,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    configuration.accept_only("exact-revision", SETTINGS)?;
    let repository = configuration.text("gitRepository")?;
    let commit = configuration.commit("gitCommit", "showcase commit")?;
    let definitions_commit = configuration
        .commit("definitionsCommit", "showcase definitions commit")?;
    let definitions_path =
        PathBuf::from(configuration.text("definitionsPath")?);
    let project = configuration.node("showcaseProject")?;
    let targets = selection(configuration)?;
    let controlling =
        pinned(&repository, &definitions_commit, cache, &project).await?;
    let definitions =
        CommittedDefinitions::fetched(&controlling, &definitions_path).await?;
    let content = pinned(&repository, &commit, cache, &project).await?;
    revision_build::collect(RevisionInputs {
        content,
        definitions,
        project,
        targets,
        publication_url,
    })
    .await
}

async fn pinned(
    repository: &str,
    commit: &str,
    cache: &Path,
    project: &NamedNode,
) -> Result<git::RepositoryScan> {
    let scan =
        git::remote_documentation(repository, commit, cache, project.as_str())
            .await?;
    ensure!(
        scan.commit == commit,
        "fetched revision {} differs from requested commit {commit}",
        scan.commit
    );
    Ok(scan)
}
