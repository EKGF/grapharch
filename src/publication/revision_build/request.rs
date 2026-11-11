//! The `publication build-revision` request: verified inputs, a staged build
//! from the restored KG only, and atomic promotion with a build record.
use {
    super::{RevisionInputs, TargetSelection},
    crate::publication::{
        collect::git::{self, CommittedDefinitions},
        dataset::Dataset,
        render::{PublicationManifest, Publisher},
        showcases,
    },
    anyhow::{Context, Result, ensure},
    clap::Args,
    oxrdf::NamedNode,
    serde::Serialize,
    sha2::{Digest, Sha256},
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

/// Explicit transport and build identity supplied by the trusted orchestrator.
#[derive(Args)]
pub struct BuildRequest {
    /// Content repository URL; its files are read as data, never executed.
    #[arg(long)]
    pub git_repository:     String,
    /// Exact designated content commit; mutable references are rejected.
    #[arg(long)]
    pub git_commit:         String,
    /// Definitions file within the controlling checkout.
    #[arg(long)]
    pub definitions:        PathBuf,
    /// Exact controlling commit; the checkout's HEAD must be this commit.
    #[arg(long)]
    pub definitions_commit: String,
    /// Project IRI defined by the controlling definitions.
    #[arg(long, value_name = "IRI")]
    pub project:            String,
    /// Publish this registered target; repeat to publish several in order.
    #[arg(
        long = "target",
        value_name = "IRI",
        required_unless_present = "all_targets",
        conflicts_with = "all_targets"
    )]
    pub targets:            Vec<String>,
    /// Publish every registered target, in IRI order.
    #[arg(long)]
    pub all_targets:        bool,
    /// Exact generator revision declared by the trusted build workflow.
    #[arg(long)]
    pub generator_revision: String,
    /// Bare Git transport cache.
    #[arg(long)]
    pub git_cache:          PathBuf,
    /// New directory, promoted only after every selected target succeeds.
    #[arg(long)]
    pub output_root:        PathBuf,
}

/// Complete build record; the executable digest identifies the actual engine.
#[derive(Serialize)]
pub struct BuildManifest {
    version:      u8,
    project:      String,
    selection:    TargetSelection,
    content:      ContentRecord,
    definitions:  DefinitionsRecord,
    generator:    GeneratorRecord,
    graph:        &'static str,
    publications: Vec<BuiltTarget>,
}

#[derive(Serialize)]
struct ContentRecord {
    repository: String,
    commit:     String,
}

#[derive(Serialize)]
struct DefinitionsRecord {
    repository: String,
    path:       String,
    commit:     String,
    blob:       String,
    sha256:     String,
}

impl DefinitionsRecord {
    fn of(definitions: &CommittedDefinitions) -> Self {
        Self {
            repository: definitions.repository().to_owned(),
            path:       definitions.path().to_owned(),
            commit:     definitions.commit().to_owned(),
            blob:       definitions.blob_id(),
            sha256:     definitions.sha256().to_owned(),
        }
    }
}

#[derive(Serialize)]
struct GeneratorRecord {
    revision:          String,
    version:           &'static str,
    executable_sha256: String,
}

#[derive(Serialize)]
struct BuiltTarget {
    destination: String,
    #[serde(flatten)]
    manifest:    PublicationManifest,
}

impl BuildRequest {
    /// Verify every input before network access, collect exact Git blobs,
    /// then render only the restored KG and promote the complete output.
    pub async fn build(&self) -> Result<BuildManifest> {
        exact_commit(&self.git_commit, "--git-commit")?;
        exact_commit(&self.definitions_commit, "--definitions-commit")?;
        exact_commit(&self.generator_revision, "--generator-revision")?;
        let project = NamedNode::new(&self.project)
            .context("--project must be an IRI")?;
        let selection = self.selection()?;
        ensure!(
            !tokio::fs::try_exists(&self.output_root).await?,
            "build output already exists"
        );
        let definitions = CommittedDefinitions::checkout(
            &self.definitions,
            &self.definitions_commit,
            project.as_str(),
        )
        .await?;
        let parent = self
            .output_root
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(parent).await?;
        let staging = parent.join(format!(
            ".grapharch-build-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir(&staging).await?;
        let staging = tokio::fs::canonicalize(staging).await?;
        let result = Box::pin(self.build_staged(
            &staging,
            project,
            selection,
            definitions,
        ))
        .await;
        match result {
            Ok(manifest) => {
                tokio::fs::rename(&staging, &self.output_root).await?;
                Ok(manifest)
            },
            Err(error) => {
                tokio::fs::remove_dir_all(&staging).await?;
                Err(error)
            },
        }
    }

    fn selection(&self) -> Result<TargetSelection> {
        if self.all_targets {
            ensure!(
                self.targets.is_empty(),
                "--all-targets conflicts with --target"
            );
            return Ok(TargetSelection::All);
        }
        TargetSelection::targets(
            self.targets
                .iter()
                .map(|target| {
                    NamedNode::new(target).with_context(|| {
                        format!("--target {target} is not an IRI")
                    })
                })
                .collect::<Result<_>>()?,
        )
    }

    async fn build_staged(
        &self,
        staging: &Path,
        project: NamedNode,
        selection: TargetSelection,
        definitions: CommittedDefinitions,
    ) -> Result<BuildManifest> {
        let content = git::remote_documentation(
            &self.git_repository,
            &self.git_commit,
            &self.git_cache,
            project.as_str(),
        )
        .await?;
        ensure!(
            content.commit == self.git_commit,
            "fetched revision differs from requested content commit"
        );
        let content_record = ContentRecord {
            repository: content.origin().to_owned(),
            commit:     content.commit.clone(),
        };
        let definitions_record = DefinitionsRecord::of(&definitions);
        let (dataset, plans) = super::collect(RevisionInputs {
            content,
            definitions,
            project: project.clone(),
            targets: selection.clone(),
        })
        .await?;
        let graph = staging.join("documentation.nq");
        dataset.export(graph.clone()).await?;
        drop(dataset);
        let restored = Dataset::memory()?;
        restored.import(graph).await?;
        let publisher = Publisher::new(restored);
        let publications =
            showcases::publish(&publisher, &plans, staging).await?;
        // bounded: a revision build selects at most 16 targets.
        let mut built = Vec::with_capacity(publications.len());
        for publication in publications {
            built.push(BuiltTarget {
                destination: publication
                    .directory
                    .strip_prefix(staging)?
                    .to_str()
                    .context("publication path must be UTF-8")?
                    .to_owned(),
                manifest:    publication.manifest,
            });
        }
        let manifest = BuildManifest {
            version: 2,
            project: project.into_string(),
            selection,
            content: content_record,
            definitions: definitions_record,
            generator: GeneratorRecord {
                revision:          self.generator_revision.clone(),
                version:           env!("CARGO_PKG_VERSION"),
                executable_sha256: executable_digest().await?,
            },
            graph: "documentation.nq",
            publications: built,
        };
        tokio::fs::write(
            staging.join("build-provenance.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )
        .await?;
        Ok(manifest)
    }
}

/// Accept only a full, lowercase, nonzero 40-character Git object id.
pub(crate) fn exact_commit(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 40 &&
            value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) &&
            value.bytes().any(|b| b != b'0'),
        "{label} requires an exact lowercase 40-character Git commit, not \
         {value:?}"
    );
    Ok(())
}

async fn executable_digest() -> Result<String> {
    let mut file = tokio::fs::File::open(std::env::current_exe()?).await?;
    let mut hash = Sha256::new();
    // bounded: one 64 KiB streaming hash buffer, off nested future frames.
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer).await?;
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    Ok(hex::encode(hash.finalize()))
}
