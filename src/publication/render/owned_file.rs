//! Ownership of single published files inside a destination directory that
//! several file-owning targets (PDF editions, RDF files) may share. Each file
//! has a record, keyed by its output-root path in the coordination area,
//! holding the owning target's manifest and any retained compiler input.
use {
    super::model::PublicationManifest,
    anyhow::{Context, Result, ensure},
    sha2::{Digest, Sha256},
    std::{
        fs,
        path::{Path, PathBuf},
    },
    uuid::Uuid,
};

/// Output-root directory for per-file publication records and staging.
pub(super) const COORDINATION: &str = ".grapharch";

/// Ownership record of one published file, keyed by its output-root path.
pub(super) struct OwnedFile {
    pub directory: PathBuf,
    lock:          PathBuf,
    name:          String,
}

/// An exclusive lease on one owned file's record; released on drop.
pub(super) struct Lease(fs::File);

impl Drop for Lease {
    fn drop(&mut self) {
        // The lease ends with the file handle when unlocking fails.
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

impl OwnedFile {
    /// The record key of a published path relative to the output root.
    pub(super) fn key(relative: &Path) -> String {
        hex::encode(Sha256::digest(
            relative.to_string_lossy().replace('\\', "/").as_bytes(),
        ))
    }

    pub(super) fn new(
        coordination: &Path,
        destination: &Path,
        name: &str,
        root: &Path,
    ) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        let relative = destination
            .strip_prefix(&root)
            .context("published file destination lies outside the output root")?
            .join(name);
        let key = Self::key(&relative);
        let files = coordination.join("files");
        fs::create_dir_all(&files)?;
        Ok(Self {
            directory: files.join(&key),
            lock:      files.join(format!("{key}.lock")),
            name:      name.to_owned(),
        })
    }

    /// Take the record's exclusive lease without waiting.
    pub(super) fn lease(&self) -> Result<Lease> {
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock)?;
        fs2::FileExt::try_lock_exclusive(&lock).with_context(|| {
            format!(
                "publication lease is unavailable: {}",
                self.lock.display()
            )
        })?;
        Ok(Lease(lock))
    }

    /// Fail fast unless the published path is absent or owned by `target`.
    pub(super) fn check_owner(
        &self,
        _lease: &Lease,
        destination: &Path,
        project: &str,
        target: &str,
    ) -> Result<()> {
        let published = destination.join(&self.name);
        if fs::symlink_metadata(&published).is_err() {
            return Ok(());
        }
        let bytes =
            fs::read(self.directory.join("manifest.yaml")).map_err(|_| {
                anyhow::anyhow!(
                    "refusing to replace an unowned file: {}",
                    published.display()
                )
            })?;
        let owner: PublicationManifest = serde_yaml::from_slice(&bytes)?;
        ensure!(
            owner.project == project && owner.target == target,
            "file name collision: {} is already published by target {}",
            published.display(),
            owner.target
        );
        ensure!(
            fs::symlink_metadata(&published)?.is_file(),
            "a published file must be a regular file"
        );
        Ok(())
    }

    /// Replace the published file and its record. `record` holds the
    /// owner's `manifest.yaml` and the `retained` files kept beside it.
    pub(super) fn replace(
        &self,
        _lease: &Lease,
        file: &Path,
        record: &Path,
        destination: &Path,
        retained: &[String],
    ) -> Result<()> {
        let next = self
            .directory
            .with_extension(format!("next-{}", Uuid::new_v4()));
        fs::create_dir(&next)?;
        for name in retained {
            let target = next.join(name);
            fs::create_dir_all(target.parent().expect("record file parent"))?;
            fs::rename(record.join(name), target)?;
        }
        fs::rename(
            record.join("manifest.yaml"),
            next.join("manifest.yaml"),
        )?;
        fs::create_dir_all(destination)?;
        fs::rename(file, destination.join(&self.name))?;
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory)?;
        }
        fs::rename(next, &self.directory)?;
        Ok(())
    }

    /// Withdraw a file its owner no longer publishes.
    pub(super) fn withdraw(
        &self,
        _lease: &Lease,
        destination: &Path,
    ) -> Result<()> {
        let published = destination.join(&self.name);
        if fs::symlink_metadata(&published).is_ok() {
            fs::remove_file(published)?;
        }
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory)?;
        }
        Ok(())
    }
}
