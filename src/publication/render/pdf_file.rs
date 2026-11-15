//! A PDF-only target owns exactly one file, its named PDF edition, inside a
//! destination directory that several PDF targets may share. Its manifest,
//! Typst source and notices live in the output root's coordination area, so
//! the shared directory holds only published PDFs.
use {
    super::{
        Publication,
        PublishOutcome,
        Publisher,
        engine,
        model::Plan,
        owned_file::{COORDINATION, OwnedFile},
    },
    anyhow::{Context, Result, ensure},
    std::{
        fs,
        path::{Path, PathBuf},
    },
    uuid::Uuid,
};

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
            let record =
                OwnedFile::new(&coordination, &destination, &name, root)?;
            let source = record.directory.clone();
            let (from, to) = (stage.clone(), destination.clone());
            let (project, target) =
                (manifest.project.clone(), manifest.target.clone());
            tokio::task::spawn_blocking(move || -> Result<()> {
                let lease = record.lease()?;
                record.check_owner(&lease, &to, &project, &target)?;
                record.replace(&lease, &from.join(&name), &from, &to, &files)
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
    let key = OwnedFile::key(relative);
    Ok(root
        .join(COORDINATION)
        .join("files")
        .join(key)
        .join("main.typ"))
}
