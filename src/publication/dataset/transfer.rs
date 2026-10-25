use {
    super::Dataset,
    anyhow::{Result, ensure},
    oxigraph::io::{RdfFormat, RdfParser},
    std::{
        io::{self, BufReader, BufWriter, Seek, SeekFrom, Write},
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
                let file = dump_to_writer(store, file)?;
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

fn dump_to_writer<W: Write>(
    store: &super::backend::BackendStore,
    writer: W,
) -> Result<W> {
    let writer = BufWriter::with_capacity(64 * 1024, writer);
    let writer = store.dump_to_writer(RdfFormat::NQuads, writer)?;
    Ok(writer.into_inner().map_err(|error| error.into_error())?)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        oxrdf::{GraphName, Literal, NamedNode, Quad},
    };

    #[derive(Default)]
    struct CountingWriter {
        calls: usize,
        bytes: usize,
    }
    impl Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            self.bytes += bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    fn large_rdf_literals_are_coalesced_into_bounded_file_writes() -> Result<()>
    {
        let store = super::super::backend::BackendStore::memory()?;
        store.insert(
            Quad::new(
                NamedNode::new("urn:test:asset")?,
                NamedNode::new("urn:test:body")?,
                Literal::new_simple_literal("x".repeat(256 * 1024)),
                GraphName::DefaultGraph,
            )
            .as_ref(),
        )?;
        let output = dump_to_writer(&store, CountingWriter::default())?;
        assert!(output.bytes >= 256 * 1024);
        assert!(
            output.calls <= 8,
            "256 KiB literal required {} file writes",
            output.calls
        );
        Ok(())
    }
    #[test]
    fn errors_flushing_the_final_export_buffer_are_reported() -> Result<()> {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("export sink failed"))
            }

            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let store = super::super::backend::BackendStore::memory()?;
        store.insert(
            Quad::new(
                NamedNode::new("urn:test:asset")?,
                NamedNode::new("urn:test:body")?,
                Literal::new_simple_literal("small buffered value"),
                GraphName::DefaultGraph,
            )
            .as_ref(),
        )?;
        assert!(dump_to_writer(&store, BrokenWriter).is_err());
        Ok(())
    }
}
