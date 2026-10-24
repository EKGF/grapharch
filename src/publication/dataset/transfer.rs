use {
    super::Dataset,
    anyhow::{Result, ensure},
    oxigraph::io::{RdfFormat, RdfParser},
    std::{
        io::{self, BufReader, Seek, SeekFrom},
        path::PathBuf,
    },
    uuid::Uuid,
};

impl Dataset {
    /// Stream a consistent dataset snapshot to N-Quads, atomically replacing
    /// the file.
    pub async fn export(&self, output: PathBuf) -> Result<()> {
        let temporary =
            output.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?
            .into_std()
            .await;
        let result = self
            .run(move |store| {
                // Audited blocking boundary: serializer and file writes run on
                // a Tokio blocking worker, without
                // materializing the dataset in memory.
                let file = store.dump_to_writer(RdfFormat::NQuads, file)?;
                file.sync_all()?;
                Ok(())
            })
            .await;
        if let Err(error) = result {
            tokio::fs::remove_file(&temporary).await?;
            return Err(error);
        }
        tokio::fs::rename(&temporary, &output).await?;
        Ok(())
    }

    /// Restore a complete exported dataset into an empty store in one
    /// transaction. Existing datasets are rejected rather than merged with
    /// conflicting ownership.
    pub async fn import(&self, input: PathBuf) -> Result<()> {
        let file = tokio::fs::File::open(input).await?.into_std().await;
        self.run(move |store| {
            ensure!(
                store.is_empty()?,
                "dataset import requires an empty store"
            );
            store.transaction::<_, io::Error>(|mut transaction| {
                // Audited blocking boundary: each retry rewinds its reader and
                // streams parser output directly into the
                // transaction; no load_from_reader Vec.
                let mut file = file.try_clone()?;
                file.seek(SeekFrom::Start(0))?;
                let mut existing =
                    transaction.quads_for_pattern(None, None, None, None);
                if existing.next().transpose()?.is_some() {
                    return Err(io::Error::other(
                        "dataset changed before import",
                    ));
                }
                drop(existing);
                for quad in RdfParser::from_format(RdfFormat::NQuads)
                    .for_reader(BufReader::new(file))
                {
                    let quad = quad.map_err(io::Error::other)?;
                    transaction.insert(quad.as_ref())?;
                }
                Ok(())
            })?;
            Ok(())
        })
        .await
    }
}
