use {
    super::{Command, ContributionStream},
    crate::publication::dataset::{
        CommittedContribution,
        MAX_CHUNK_BYTES,
        MAX_CHUNK_QUADS,
    },
    anyhow::{Result, ensure},
    oxrdf::Quad,
    tokio::sync::oneshot,
};

impl ContributionStream {
    /// Send and acknowledge a bounded chunk; the mutable receiver enforces
    /// producer ordering.
    pub async fn push(&mut self, quads: Vec<Quad>) -> Result<()> {
        ensure!(
            !self.failed,
            "contribution stream has already failed"
        );
        self.flush().await?;
        let result = self.send_chunk(quads).await;
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Bind the observed source revision to this still-current attempt.
    pub async fn set_revision(&mut self, revision: &str) -> Result<()> {
        ensure!(
            !self.failed,
            "contribution stream has already failed"
        );
        let size = revision.len() + 256;
        ensure!(
            size <= MAX_CHUNK_BYTES && size <= self.writer.byte_limit,
            "revision metadata exceeds byte limit"
        );
        let permit = self
            .writer
            .bytes
            .clone()
            .acquire_many_owned(size as u32)
            .await?;
        let (reply, response) = oneshot::channel();
        self.writer
            .sender
            .send(Command::Refine {
                attempt: self.attempt.clone(),
                revision: revision.to_owned(),
                _permit: permit,
                reply,
            })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        self.attempt = response.await.map_err(|_| {
            anyhow::anyhow!(
                "graph writer stopped before revision acknowledgement"
            )
        })??;
        self.needs_revision = false;
        Ok(())
    }

    /// Buffer one quad, splitting chunks on both byte count and quad count.
    pub async fn write(&mut self, quad: Quad) -> Result<()> {
        ensure!(
            !self.failed,
            "contribution stream has already failed"
        );
        let size = quad.to_string().len() + 256;
        if size > MAX_CHUNK_BYTES || size > self.writer.byte_limit {
            self.failed = true;
            anyhow::bail!("quad exceeds byte limit");
        }
        if self.pending.len() == MAX_CHUNK_QUADS ||
            self.pending_bytes + size > MAX_CHUNK_BYTES ||
            self.pending_bytes + size > self.writer.byte_limit
        {
            self.flush().await?;
        }
        self.pending_bytes += size;
        self.pending.push(quad);
        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let quads = std::mem::take(&mut self.pending);
        self.pending_bytes = 0;
        let result = self.send_chunk(quads).await;
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    async fn send_chunk(&self, quads: Vec<Quad>) -> Result<()> {
        ensure!(
            quads.len() <= MAX_CHUNK_QUADS,
            "chunk exceeds quad limit"
        );
        let mut size = 0usize;
        for quad in &quads {
            size = size
                .checked_add(quad.to_string().len() + 256)
                .ok_or_else(|| anyhow::anyhow!("chunk size overflow"))?;
            ensure!(
                size <= MAX_CHUNK_BYTES && size <= self.writer.byte_limit,
                "chunk exceeds byte limit"
            );
        }
        let permit = self
            .writer
            .bytes
            .clone()
            .acquire_many_owned(size as u32)
            .await?;
        let (reply, response) = oneshot::channel();
        self.writer
            .sender
            .send(Command::Chunk {
                attempt: self.attempt.clone(),
                quads,
                _permit: permit,
                reply,
            })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        response.await.map_err(|_| {
            anyhow::anyhow!("graph writer stopped before chunk acknowledgement")
        })?
    }

    /// Promote only after every preceding chunk has been acknowledged.
    pub async fn complete(self) -> Result<()> {
        self.commit().await.map(|_| ())
    }

    /// Commit and return a receipt that can be pinned without consulting
    /// mutable heads.
    pub async fn commit(mut self) -> Result<CommittedContribution> {
        ensure!(!self.failed, "cannot complete a failed stream");
        ensure!(
            !self.needs_revision,
            "pending contribution requires an observed source revision"
        );
        self.flush().await?;
        let receipt = self.attempt.receipt();
        let (reply, response) = oneshot::channel();
        self.writer
            .sender
            .send(Command::Complete { attempt: self.attempt, reply })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        response.await.map_err(|_| {
            anyhow::anyhow!(
                "graph writer stopped before completion acknowledgement"
            )
        })??;
        Ok(receipt)
    }

    /// Explicitly discard this attempt without replacing the active
    /// contribution.
    pub async fn abort(self) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.writer
            .sender
            .send(Command::Abort { attempt: self.attempt, reply })
            .await
            .map_err(|_| anyhow::anyhow!("graph writer closed"))?;
        response.await.map_err(|_| {
            anyhow::anyhow!("graph writer stopped before abort acknowledgement")
        })?
    }
}
