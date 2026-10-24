//! Audited blocking boundary for short, serialized filesystem publication
//! transactions. Lease acquisition, ticket checks, directory promotion, and
//! rollback run together on a blocking worker, including cancellation cleanup.

use {
    super::model::{Plan, PublicationManifest},
    anyhow::{Context, Result, ensure},
    serde::{Deserialize, Serialize},
    std::{
        fs,
        io::{Read, Write},
        path::{Component, Path, PathBuf},
        sync::Arc,
    },
    tokio::sync::Mutex,
    uuid::Uuid,
};

const METADATA_LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Ticket {
    pub session:  String,
    pub sequence: u64,
    pub job:      String,
    pub project:  String,
    pub target:   String,
}

pub(super) async fn prepare(root: &Path, plan: &Plan) -> Result<PathBuf> {
    tokio::fs::create_dir_all(root).await?;
    let root = tokio::fs::canonicalize(root).await?;
    let relative = Path::new(&plan.destination);
    ensure!(
        !relative.as_os_str().is_empty() &&
            relative
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "invalid target destination"
    );
    let destination = root.join(relative);
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("target has no parent"))?;
    let mut current = root;
    for component in parent.strip_prefix(&current)?.components() {
        current.push(component);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) => {
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "publication parent must be a real directory: {}",
                    current.display()
                )
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir(&current).await?
            },
            Err(error) => return Err(error.into()),
        }
        ensure!(
            !tokio::fs::try_exists(current.join("manifest.yaml")).await?,
            "nested target outputs require explicit composition"
        );
    }
    Ok(destination)
}

fn paths(destination: &Path) -> Result<(PathBuf, PathBuf)> {
    let name = destination
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("missing destination name"))?
        .to_string_lossy();
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing destination parent"))?;
    Ok((
        parent.join(format!(".grapharch-{name}.lock")),
        parent.join(format!(".grapharch-{name}.request.yaml")),
    ))
}

pub(super) async fn register(
    destination: PathBuf,
    ticket: Ticket,
    coordination: Arc<Mutex<()>>,
) -> Result<bool> {
    let guard = coordination.lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        let (lock, request) = paths(&destination)?;
        let lease = Lease::acquire(lock)?;
        if let Some(previous) = read_optional::<Ticket>(&request)? {
            ensure!(
                previous.project == ticket.project &&
                    previous.target == ticket.target,
                "another target owns this output destination"
            );
            recover(&destination, &previous)?;
            if previous.session == ticket.session &&
                previous.sequence > ticket.sequence
            {
                lease.release()?;
                return Ok(false);
            }
        }
        owned(&destination, &ticket.project, &ticket.target)?;
        let temporary =
            request.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        output.write_all(serde_yaml::to_string(&ticket)?.as_bytes())?;
        output.sync_all()?;
        fs::rename(temporary, request)?;
        lease.release()?;
        Ok(true)
    })
    .await?
}

pub(super) async fn promote(
    stage: PathBuf,
    destination: PathBuf,
    ticket: Ticket,
    coordination: Arc<Mutex<()>>,
) -> Result<bool> {
    let guard = coordination.lock_owned().await;
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        let (lock, request) = paths(&destination)?;
        let lease = Lease::acquire(lock)?;
        let current = read_optional::<Ticket>(&request)?
            .ok_or_else(|| anyhow::anyhow!("missing publication request"))?;
        if current.job != ticket.job {
            lease.release()?;
            return Ok(false);
        }
        let existing = owned(&destination, &ticket.project, &ticket.target)?;
        let backup = backup_path(&destination, &ticket.job)?;
        if existing {
            fs::rename(&destination, &backup)?;
        }
        if let Err(error) = fs::rename(&stage, &destination) {
            if existing {
                fs::rename(&backup, &destination).context(
                    "publication promotion failed and rollback could not \
                     restore the previous output",
                )?;
            }
            return Err(error.into());
        }
        if existing {
            fs::remove_dir_all(backup)?;
        }
        lease.release()?;
        Ok(true)
    })
    .await?
}

fn backup_path(destination: &Path, job: &str) -> Result<PathBuf> {
    let job = Uuid::parse_str(job)?;
    Ok(destination.with_file_name(format!(".grapharch-backup-{job}")))
}

fn recover(destination: &Path, previous: &Ticket) -> Result<()> {
    let backup = backup_path(destination, &previous.job)?;
    if !backup.try_exists()? {
        return Ok(());
    }
    ensure!(
        owned(&backup, &previous.project, &previous.target)?,
        "interrupted promotion has no owned backup"
    );
    if destination.try_exists()? {
        ensure!(
            owned(destination, &previous.project, &previous.target)?,
            "interrupted promotion has an unowned destination"
        );
        let manifest = read_optional::<PublicationManifest>(
            &destination.join("manifest.yaml"),
        )?
        .ok_or_else(|| anyhow::anyhow!("missing promotion manifest"))?;
        ensure!(
            manifest.job == previous.job,
            "interrupted publication has inconsistent generation metadata"
        );
        fs::remove_dir_all(backup)?;
    } else {
        fs::rename(backup, destination)?;
    }
    Ok(())
}

fn owned(destination: &Path, project: &str, target: &str) -> Result<bool> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "output destination must be a real directory"
            )
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false)
        },
        Err(error) => return Err(error.into()),
    }
    let manifest = read_optional::<PublicationManifest>(
        &destination.join("manifest.yaml"),
    )?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "refusing to replace an unowned output directory: {}",
            destination.display()
        )
    })?;
    ensure!(
        manifest.version == 1 &&
            manifest.project == project &&
            manifest.target == target,
        "another publication owns this output directory"
    );
    // bounded: only the renderer's fixed file set is accepted, never arbitrary
    // paths.
    ensure!(
        manifest.files.len() <= 4 &&
            manifest.files.iter().all(|name| {
                matches!(
                    name.as_str(),
                    "index.html" | "style.css" | "main.typ" | "document.pdf"
                )
            }),
        "invalid publication artifact ownership"
    );
    for entry in fs::read_dir(destination)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("unowned non-UTF-8 output path"))?;
        ensure!(
            entry.file_type()?.is_file() &&
                (name == "manifest.yaml" ||
                    manifest.files.iter().any(|owned| owned == name)),
            "output contains an unowned artifact: {}",
            entry.path().display()
        );
    }
    Ok(true)
}

fn read_optional<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<Option<T>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None)
        },
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new(); // bounded: METADATA_LIMIT + 1.
    file.take(METADATA_LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= METADATA_LIMIT,
        "publication metadata exceeds byte limit"
    );
    Ok(Some(serde_yaml::from_slice(&bytes)?))
}

struct Lease {
    file: fs::File,
}

impl Lease {
    fn acquire(path: PathBuf) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        fs2::FileExt::try_lock_exclusive(&file).with_context(|| {
            format!(
                "publication lease is unavailable: {}",
                path.display()
            )
        })?;
        Ok(Self { file })
    }

    fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.file)?;
        Ok(())
    }
}

pub(super) struct Staging {
    pub path: PathBuf,
}

impl Staging {
    pub async fn new(destination: &Path) -> Result<Self> {
        let path = destination
            .with_file_name(format!(".grapharch-stage-{}", Uuid::new_v4()));
        tokio::fs::create_dir(&path).await?;
        Ok(Self { path })
    }

    pub async fn cleanup(&self) -> Result<()> {
        match tokio::fs::remove_dir_all(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(())
            },
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let path = self.path.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = tokio::fs::remove_dir_all(&path).await {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        tracing::error!(
                            "cannot remove abandoned staging directory {}: \
                             {error}",
                            path.display()
                        );
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_lock_file_from_a_previous_process_does_not_block_publication()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("publication.lock");
        fs::write(&path, b"")?;
        let first = Lease::acquire(path.clone())?;
        assert!(Lease::acquire(path.clone()).is_err());
        first.release()?;
        assert!(path.is_file());
        Lease::acquire(path)?.release()?;
        Ok(())
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[tokio::test]
    async fn interrupted_promotion_restores_the_previous_generation()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        let destination = root.path().join("spec");
        tokio::fs::create_dir(&destination).await?;
        let manifest = PublicationManifest {
            version:        1,
            project:        "urn:project".into(),
            target:         "urn:target".into(),
            input_revision: "urn:revision".into(),
            plan:           "urn:plan".into(),
            plan_snapshot:  "urn:snapshot".into(),
            scope:          "urn:scope".into(),
            profile:        "urn:profile".into(),
            renderer:       "html".into(),
            job:            Uuid::new_v4().to_string(),
            diagnostics:    Vec::new(),
            failure_mode:   Default::default(),
            files:          vec!["index.html".into()], /* bounded: the
                                                        * fixture's single
                                                        * artifact. */
        };
        tokio::fs::write(
            destination.join("manifest.yaml"),
            serde_yaml::to_string(&manifest)?,
        )
        .await?;
        tokio::fs::write(
            destination.join("index.html"),
            "Previous valid page",
        )
        .await?;
        let first = Ticket {
            session:  Uuid::new_v4().to_string(),
            sequence: 1,
            job:      Uuid::new_v4().to_string(),
            project:  "urn:project".into(),
            target:   "urn:target".into(),
        };
        let coordination = Arc::new(Mutex::new(()));
        assert!(
            register(
                destination.clone(),
                first.clone(),
                coordination.clone()
            )
            .await?
        );
        let backup = destination
            .with_file_name(format!(".grapharch-backup-{}", first.job));
        tokio::fs::rename(&destination, &backup).await?;
        let next = Ticket {
            sequence: 2,
            job: Uuid::new_v4().to_string(),
            ..first
        };
        assert!(register(destination.clone(), next, coordination).await?);
        assert_eq!(
            tokio::fs::read_to_string(destination.join("index.html")).await?,
            "Previous valid page"
        );
        assert!(!tokio::fs::try_exists(backup).await?);
        Ok(())
    }
}
