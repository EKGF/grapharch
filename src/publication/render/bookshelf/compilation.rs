//! Recording of one book worker outcome against its book.
use {
    super::{Book, file_hash},
    crate::publication::diagnostics::{Diagnostic, FailureMode, Severity},
    anyhow::{Result, ensure},
    std::path::Path,
};

impl Book {
    /// Record one worker outcome for this book. Fail-fast mode turns a failed
    /// compilation into an error; otherwise it becomes a diagnostic.
    pub(super) async fn record_compilation(
        &mut self,
        stage: &Path,
        result: Result<()>,
        mode: FailureMode,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        let error = match result {
            Ok(()) => {
                let pdf = self.source.replace(".typ", ".pdf");
                self.pdf_digest = Some(
                    file_hash(
                        &stage.join(&pdf),
                        crate::publication::limits::ARTIFACT_BYTES,
                    )
                    .await?,
                );
                self.pdf = Some(pdf);
                self.status = "ready".into();
                tracing::info!(book=%self.title,subject=%self.id,"book PDF completed");
                return Ok(());
            },
            Err(error) => error,
        };
        if mode == FailureMode::FailFast {
            return Err(error.context(format!(
                "PDF generation failed for book {} ({})",
                self.title, self.id
            )));
        }
        ensure!(
            diagnostics.len() < 1024,
            "book diagnostic budget exceeded"
        );
        let partial = stage.join(&self.source).with_extension("pdf");
        match tokio::fs::symlink_metadata(&partial).await {
            Ok(meta) => {
                ensure!(
                    meta.is_file() && !meta.file_type().is_symlink(),
                    "worker left an invalid partial PDF"
                );
                tokio::fs::remove_file(partial).await?;
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.into()),
        }
        self.status = "failed".into();
        diagnostics.push(Diagnostic {
            severity:        Severity::Error,
            code:            "book-render-failed".into(),
            message:         error.to_string(),
            reference:       self.id.clone(),
            source_fragment: None,
        });
        Ok(())
    }
}
