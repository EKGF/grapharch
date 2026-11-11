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
    },
    anyhow::{Context, Result, ensure},
    oxrdf::NamedNode,
    serde::Deserialize,
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

const CONFIGURATION_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    repository:  String,
    commit:      String,
    definitions: Definitions,
    project:     String,
    targets:     Targets,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definitions {
    commit: String,
    path:   PathBuf,
}

/// `all`, or an ordered list of target IRIs.
#[derive(Deserialize)]
#[serde(untagged)]
enum Targets {
    All(String),
    Selected(Vec<String>),
}

impl Targets {
    fn selection(&self) -> Result<TargetSelection> {
        match self {
            Self::All(all) => {
                ensure!(
                    all == "all",
                    "showcase targets must be `all` or a list of target IRIs"
                );
                Ok(TargetSelection::All)
            },
            Self::Selected(targets) => {
                TargetSelection::targets(
                    targets
                        .iter()
                        .map(|target| {
                            NamedNode::new(target).with_context(|| {
                                format!(
                                    "showcase target {target} is not an IRI"
                                )
                            })
                        })
                        .collect::<Result<_>>()?,
                )
            },
        }
    }
}

/// Collect and plan one exact-revision showcase. Mutable references are
/// rejected before any network access or cache creation.
pub(super) async fn build(
    configuration: &Path,
    cache: &Path,
    publication_url: PublicationUrl,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let mut bytes = Vec::new();
    tokio::fs::File::open(configuration)
        .await?
        .take(CONFIGURATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() as u64 <= CONFIGURATION_BYTES,
        "revision showcase configuration exceeds 64 KiB"
    );
    let source: Source = serde_yaml::from_slice(&bytes)?;
    revision_build::exact_commit(&source.commit, "showcase commit")?;
    revision_build::exact_commit(
        &source.definitions.commit,
        "showcase definitions commit",
    )?;
    let project = NamedNode::new(&source.project)
        .context("showcase project must be an IRI")?;
    let targets = source.targets.selection()?;
    let controlling = pinned(
        &source.repository,
        &source.definitions.commit,
        cache,
        &project,
    )
    .await?;
    let definitions =
        CommittedDefinitions::fetched(&controlling, &source.definitions.path)
            .await?;
    let content = pinned(
        &source.repository,
        &source.commit,
        cache,
        &project,
    )
    .await?;
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
