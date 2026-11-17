//! Selected binary assets and the retained publication inventory artifact.
use {
    super::{artifact, page::PageWriter},
    anyhow::{Result, ensure},
};

impl PageWriter<'_> {
    /// Copy every selected plan asset into the stage, within the retained
    /// asset and image budgets, then the publication inventory if configured.
    pub(super) async fn write_assets(
        self,
        files: &mut Vec<String>,
    ) -> Result<()> {
        let Self { plan, stage, catalog, .. } = self;
        let mut assets = plan.assets().await?;
        let mut asset_bytes = 0u64;
        let mut image_bytes = 0u64;
        while let Some(row) = assets.next().await? {
            let id = crate::publication::render::model::node(&row["asset"])?;
            let asset = catalog.page(id)?;
            ensure!(
                asset.binary,
                "selected publication asset must be binary"
            );
            asset_bytes +=
                plan.source.text(id, "contentSize").await?.parse::<u64>()?;
            ensure!(
                asset_bytes <= crate::publication::limits::RETAINED_ASSET_BYTES,
                "website assets exceed 512 MiB budget"
            );
            let image = plan.source.optional(id, "format").await?;
            if let Some(oxrdf::Term::Literal(mime)) = &image &&
                mime.value().starts_with("image/")
            {
                let size: u64 =
                    plan.source.text(id, "contentSize").await?.parse()?;
                ensure!(
                    size <= crate::publication::image_assets::MAX_IMAGE_BYTES
                        as u64,
                    "image exceeds byte budget"
                );
                image_bytes += size;
                ensure!(image_bytes <= crate::publication::image_assets::MAX_PUBLICATION_IMAGE_BYTES as u64, "publication images exceed total byte budget");
            }
            let output = stage.join(&asset.route);
            tokio::fs::create_dir_all(
                output
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("artifact has no parent"))?,
            )
            .await?;
            artifact::write(plan, id, &output).await?;
            if let Some(oxrdf::Term::Literal(mime)) = image &&
                mime.value().starts_with("image/")
            {
                let size = tokio::fs::metadata(&output).await?.len();
                ensure!(
                    size <= crate::publication::image_assets::MAX_IMAGE_BYTES
                        as u64,
                    "image exceeds byte budget"
                );
                crate::publication::image_assets::validate(
                    &tokio::fs::read(&output).await?,
                    mime.value(),
                )?;
            }
            files.push(asset.route.clone());
        }
        if let Some(job) = crate::publication::document::publication_inventory::model::configured_job(&plan.dataset, &plan.source, &plan.project).await? {
            ensure!(!files.iter().any(|path| path == "publication-versions.json"), "publication inventory artifact collides with authored content");
            let inventory = crate::publication::document::publication_inventory::model::Inventory::read(&plan.dataset, &plan.source, &job.subject).await?;
            tokio::fs::write(stage.join("publication-versions.json"), serde_json::to_vec_pretty(&inventory)?).await?;
            files.push("publication-versions.json".into());
        }
        Ok(())
    }
}
