//! A PDF-only target owns exactly one file, its named PDF edition, inside a
//! destination directory that several PDF targets may share. Its manifest,
//! Typst source and notices live in the output root's coordination area, so
//! the shared directory holds only published PDFs.
use {
    super::{Publication, PublishOutcome, Publisher, engine, model::Plan},
    crate::publication::render::model::PublicationManifest,
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

impl Publisher {
    pub(super) async fn publish_pdf_file(
        &self,
        plan: &Plan,
        root: &Path,
        destination: PathBuf,
        job: &str,
    ) -> Result<PublishOutcome> {
        let name = plan
            .pdf_file
            .clone()
            .ok_or_else(|| anyhow::anyhow!("PDF target has no file name"))?;
        ensure!(
            !tokio::fs::try_exists(destination.join("manifest.yaml")).await?,
            "PDF target {} cannot publish {name} into {}: that directory is \
             owned by a directory publication",
            plan.target,
            destination.display()
        );
        let coordination =
            tokio::fs::canonicalize(root).await?.join(COORDINATION);
        tokio::fs::create_dir_all(&coordination).await?;
        let stage = coordination.join(format!("stage-{}", Uuid::new_v4()));
        tokio::fs::create_dir(&stage).await?;
        let result = async {
            let mut files = plan.images.write(plan, &stage).await?;
            let theme = super::typst::theme(plan).await?;
            files.extend(super::cover::write_logo(plan, &theme, &stage).await?);
            let diagnostics =
                super::content::write(plan, &stage, self.failure_mode).await?;
            let pdf = self.compile(&stage).await?;
            tokio::fs::write(stage.join(&name), pdf).await?;
            files.push(crate::publication::icon::write_licenses(&stage).await?);
            files.push("main.typ".into());
            let mut manifest = plan.manifest(job);
            manifest.files = vec![name.clone()];
            manifest.pdf = Some(name.clone());
            manifest.diagnostics = diagnostics;
            manifest.failure_mode = self.failure_mode;
            tokio::fs::write(
                stage.join("manifest.yaml"),
                serde_yaml::to_string(&manifest)?,
            )
            .await?;
            let record = Record::new(&coordination, &destination, &name, root)?;
            let source = record.directory.clone();
            let (from, to) = (stage.clone(), destination.clone());
            let (project, target) =
                (manifest.project.clone(), manifest.target.clone());
            tokio::task::spawn_blocking(move || {
                record.promote(&from, &to, &project, &target, &files)
            })
            .await??;
            Ok(PublishOutcome::Published(Box::new(Publication {
                source,
                directory: destination.clone(),
                manifest,
            })))
        }
        .await;
        match tokio::fs::remove_dir_all(&stage).await {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.into());
            },
            _ => {},
        }
        result
    }

    pub(super) async fn compile(&self, stage: &Path) -> Result<Vec<u8>> {
        ensure!(
            tokio::fs::metadata(stage.join("main.typ")).await?.len() <=
                crate::publication::limits::TYPST_SOURCE_BYTES,
            "Typst source exceeds compilation budget"
        );
        let assets = self
            .assets
            .get_or_try_init(engine::Assets::load)
            .await?
            .clone();
        let pdf = engine::compile(
            stage.to_owned(),
            assets,
            self.compile_slots.clone(),
        )
        .await?;
        ensure!(
            pdf.len() as u64 <= crate::publication::limits::ARTIFACT_BYTES,
            "PDF exceeds artifact byte budget"
        );
        Ok(pdf)
    }
}

/// The retained Typst source of a PDF edition published by a PDF-only target,
/// given the output root and the PDF's path inside it.
pub fn typst_source(root: &Path, pdf: &Path) -> Result<PathBuf> {
    let root = fs::canonicalize(root)?;
    let pdf = fs::canonicalize(pdf)?;
    let relative = pdf
        .strip_prefix(&root)
        .context("PDF lies outside the output root")?;
    let key = hex::encode(Sha256::digest(
        relative.to_string_lossy().replace('\\', "/").as_bytes(),
    ));
    Ok(root
        .join(COORDINATION)
        .join("files")
        .join(key)
        .join("main.typ"))
}

/// Ownership record of one published file, keyed by its output-root path.
struct Record {
    directory: PathBuf,
    lock:      PathBuf,
    name:      String,
}

impl Record {
    fn new(
        coordination: &Path,
        destination: &Path,
        name: &str,
        root: &Path,
    ) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        let relative = destination
            .strip_prefix(&root)
            .context("PDF destination lies outside the output root")?
            .join(name);
        let key = hex::encode(Sha256::digest(
            relative.to_string_lossy().replace('\\', "/").as_bytes(),
        ));
        let files = coordination.join("files");
        fs::create_dir_all(&files)?;
        Ok(Self {
            directory: files.join(&key),
            lock:      files.join(format!("{key}.lock")),
            name:      name.to_owned(),
        })
    }

    /// Replace the owned file atomically, failing fast when another target
    /// or no publication owns the path.
    fn promote(
        &self,
        stage: &Path,
        destination: &Path,
        project: &str,
        target: &str,
        retained: &[String],
    ) -> Result<()> {
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
        let published = destination.join(&self.name);
        let previous = self.directory.join("manifest.yaml");
        if fs::symlink_metadata(&published).is_ok() {
            let bytes = fs::read(&previous).map_err(|_| {
                anyhow::anyhow!(
                    "refusing to replace an unowned file: {}",
                    published.display()
                )
            })?;
            let owner: PublicationManifest = serde_yaml::from_slice(&bytes)?;
            ensure!(
                owner.project == project && owner.target == target,
                "PDF file name collision: {} is already published by target {}",
                published.display(),
                owner.target
            );
            ensure!(
                fs::symlink_metadata(&published)?.is_file(),
                "published PDF must be a regular file"
            );
        }
        let next = self
            .directory
            .with_extension(format!("next-{}", Uuid::new_v4()));
        fs::create_dir(&next)?;
        // The record keeps the complete compiler input beside the manifest.
        for name in retained {
            let target = next.join(name);
            fs::create_dir_all(target.parent().expect("record file parent"))?;
            fs::rename(stage.join(name), target)?;
        }
        fs::rename(
            stage.join("manifest.yaml"),
            next.join("manifest.yaml"),
        )?;
        fs::create_dir_all(destination)?;
        fs::rename(stage.join(&self.name), &published)?;
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory)?;
        }
        fs::rename(next, &self.directory)?;
        fs2::FileExt::unlock(&lock)?;
        Ok(())
    }
}
