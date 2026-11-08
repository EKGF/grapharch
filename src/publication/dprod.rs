//! Exact-revision publication without a showcase registry or source execution.
use {
    super::{
        collect::git,
        dataset::Dataset,
        render::{PublicationManifest, Publisher},
        showcases,
        showcases_dprod,
    },
    anyhow::{Result, ensure},
    clap::Args,
    serde::Serialize,
    sha2::{Digest, Sha256},
    std::path::{Path, PathBuf},
    tokio::io::AsyncReadExt,
};

/// Explicit transport and build identity supplied by the trusted orchestrator.
#[derive(Args)]
pub struct BuildRequest {
    /// Repository URL; repository contents are read as data, never executed.
    #[arg(long)]
    pub git_repository:     String,
    /// Full source commit; mutable branch names are rejected.
    #[arg(long)]
    pub git_ref:            String,
    /// Source-owned definition file within that same commit.
    #[arg(long)]
    pub definitions:        String,
    /// Exact generator revision declared by the trusted build workflow.
    #[arg(long)]
    pub generator_revision: String,
    /// Bare Git transport cache.
    #[arg(long)]
    pub git_cache:          PathBuf,
    /// New directory promoted only after every declared target succeeds.
    #[arg(long)]
    pub output_root:        PathBuf,
}

/// Complete build record; the engine digest identifies the actual executable.
#[derive(Serialize)]
pub struct BuildManifest {
    version:            u8,
    source_repository:  String,
    source_commit:      String,
    definitions:        String,
    definitions_commit: String,
    generator_revision: String,
    generator_version:  &'static str,
    executable_sha256:  String,
    graph:              &'static str,
    publications:       Vec<BuiltTarget>,
}

#[derive(Serialize)]
struct BuiltTarget {
    destination: String,
    #[serde(flatten)]
    manifest:    PublicationManifest,
}

impl BuildRequest {
    /// Collect and resolve exact Git blobs, then render only the restored KG.
    pub async fn build(&self) -> Result<BuildManifest> {
        exact_commit(&self.git_ref)?;
        exact_commit(&self.generator_revision)?;
        ensure!(
            !tokio::fs::try_exists(&self.output_root).await?,
            "build output already exists"
        );
        let parent = self
            .output_root
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(parent).await?;
        let staging = parent.join(format!(
            ".grapharch-build-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir(&staging).await?;
        let staging = tokio::fs::canonicalize(staging).await?;
        let result = self.build_staged(&staging).await;
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

    async fn build_staged(&self, staging: &Path) -> Result<BuildManifest> {
        let scan = git::remote_repository(
            &self.git_repository,
            &self.git_ref,
            &self.git_cache,
            "urn:dprod-showcase:project",
        )
        .await?;
        ensure!(
            scan.commit == self.git_ref,
            "fetched revision differs from requested source commit"
        );
        let (dataset, plans) =
            showcases_dprod::collect_scan(scan, &self.definitions).await?;
        let graph = staging.join("documentation.nq");
        dataset.export(graph.clone()).await?;
        drop(dataset);
        let restored = Dataset::memory()?;
        restored.import(graph).await?;
        let publisher = Publisher::new(restored);
        let publications =
            showcases::publish(&publisher, &plans, staging).await?;
        // bounded: the source-owned adapter admits at most 16 targets.
        let mut targets = Vec::with_capacity(publications.len());
        for publication in publications {
            targets.push(BuiltTarget {
                destination: publication
                    .directory
                    .strip_prefix(staging)?
                    .to_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("publication path must be UTF-8")
                    })?
                    .to_owned(),
                manifest:    publication.manifest,
            });
        }
        let manifest = BuildManifest {
            version:            1,
            source_repository:  self.git_repository.clone(),
            source_commit:      self.git_ref.clone(),
            definitions:        self.definitions.clone(),
            definitions_commit: self.git_ref.clone(),
            generator_revision: self.generator_revision.clone(),
            generator_version:  env!("CARGO_PKG_VERSION"),
            executable_sha256:  executable_digest().await?,
            graph:              "documentation.nq",
            publications:       targets,
        };
        tokio::fs::write(
            staging.join("build-provenance.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )
        .await?;
        Ok(manifest)
    }
}

fn exact_commit(value: &str) -> Result<()> {
    ensure!(
        value.len() == 40 &&
            value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) &&
            value.bytes().any(|b| b != b'0'),
        "an exact lowercase 40-character Git commit is required"
    );
    Ok(())
}

async fn executable_digest() -> Result<String> {
    let mut file = tokio::fs::File::open(std::env::current_exe()?).await?;
    let mut hash = Sha256::new();
    // bounded: one 64 KiB streaming hash buffer; keep it off nested future
    // frames.
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
