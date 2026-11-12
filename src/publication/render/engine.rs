//! The audited synchronous Typst boundary. All functions execute on bounded
//! blocking workers; no source graph or corpus is hydrated into this world.

use {
    super::pdf_standard::PdfConformance,
    anyhow::{Result, ensure},
    std::{path::PathBuf, sync::Arc},
    typst::{
        Library,
        LibraryExt,
        World,
        diag::FileError,
        foundations::{Bytes, Datetime, Duration},
        syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
        text::{Font, FontBook},
        utils::LazyHash,
    },
    typst_kit::fonts::{FontStore, embedded},
    typst_layout::PagedDocument,
};

pub(super) struct Assets {
    library: LazyHash<Library>,
    // bounded: fixed embedded font assets, independent of graph/document size.
    fonts:   FontStore,
}

impl Assets {
    pub async fn load() -> Result<Arc<Self>> {
        tokio::task::spawn_blocking(|| {
            let mut fonts = FontStore::new();
            fonts.extend(embedded());
            ensure!(
                fonts.font(0).is_some(),
                "Typst requires the embedded font assets"
            );
            let liberation = super::fonts::EMBEDDED
                .into_iter()
                .map(|bytes| {
                    Font::new(Bytes::new(bytes), 0)
                        .map(|font| {
                            let info = font.info().clone();
                            (font, info)
                        })
                        .ok_or_else(|| {
                            anyhow::anyhow!("invalid embedded PDF font")
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            fonts.extend(liberation);
            Ok(Arc::new(Self {
                library: LazyHash::new(Library::default()),
                fonts,
            }))
        })
        .await?
    }
}

struct DocumentWorld {
    root:   PathBuf,
    assets: Arc<Assets>,
    main:   FileId,
}

impl DocumentWorld {
    /// Resolves a project file below the publication root. Packages are never
    /// fetched: the rendered source is self-contained.
    fn path(&self, id: FileId) -> std::result::Result<PathBuf, FileError> {
        match id.root() {
            VirtualRoot::Project => Ok(id.vpath().realize(&self.root)?),
            VirtualRoot::Package(_) => {
                Err(FileError::NotFound(PathBuf::from(
                    id.vpath().get_without_slash(),
                )))
            },
        }
    }
}

impl World for DocumentWorld {
    fn library(&self) -> &LazyHash<Library> { &self.assets.library }

    fn book(&self) -> &LazyHash<FontBook> { self.assets.fonts.book() }

    fn main(&self) -> FileId { self.main }

    fn source(&self, id: FileId) -> std::result::Result<Source, FileError> {
        let path = self.path(id)?;
        std::fs::read_to_string(&path)
            .map(|source| Source::new(id, source))
            .map_err(|error| FileError::from_io(error, &path))
    }

    fn file(&self, id: FileId) -> std::result::Result<Bytes, FileError> {
        let path = self.path(id)?;
        std::fs::read(&path)
            .map(Bytes::new)
            .map_err(|error| FileError::from_io(error, &path))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.assets.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> { None }
}

pub(super) async fn compile(
    root: PathBuf,
    assets: Arc<Assets>,
    slots: Arc<tokio::sync::Semaphore>,
) -> Result<Vec<u8>> {
    compile_file(root, "main.typ".into(), assets, slots).await
}

pub(super) async fn compile_file(
    root: PathBuf,
    main: String,
    assets: Arc<Assets>,
    slots: Arc<tokio::sync::Semaphore>,
) -> Result<Vec<u8>> {
    let permit = slots.acquire_owned().await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let world = DocumentWorld {
            root,
            assets,
            main: FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new(&main).map_err(|error| {
                    anyhow::anyhow!("invalid Typst main file {main}: {error}")
                })?,
            )),
        };
        let started = std::time::Instant::now();
        let result = typst::compile::<PagedDocument>(&world);
        let document = result.output.map_err(|diagnostics| {
            anyhow::anyhow!("Typst compilation failed: {diagnostics:?}")
        })?;
        ensure!(
            result.warnings.is_empty(),
            "Typst compilation warnings: {:?}",
            result.warnings
        );
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            pages = document.pages().len(),
            "Typst document laid out"
        );
        let pdf_started = std::time::Instant::now();
        let options = PdfConformance::of(&document)?.options()?;
        let pdf =
            typst_pdf::pdf(&document, &options).map_err(|diagnostics| {
                anyhow::anyhow!("PDF generation failed: {diagnostics:?}")
            })?;
        tracing::info!(
            elapsed_ms = pdf_started.elapsed().as_millis(),
            bytes = pdf.len(),
            "PDF encoded"
        );
        Ok(pdf)
    })
    .await?
}

#[cfg(test)]
mod ribbon_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "native performance acceptance: run with cargo test --release \
                --lib large_book_running_headers -- --ignored"]
    async fn large_book_running_headers_complete_without_per_page_chapter_scans()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = format!(
            "#let ga-style = \"specification\"\n#let ga-accent = \
             rgb(\"#2b5f9f\")\n#let ga-title = \"Running header scale \
             test\"\n#let ga-publisher = \"Example Authority\"\n#let \
             ga-ribbon = none\n{}\n#for chapter in range(128) {{ \
             heading(level: 1)[#text(\"Chapter \" + str(chapter))]; for term \
             in range(128) {{ heading(level: 6, numbering: none)[#text(\"Term \
             \" + str(term))]; [A bounded header performance fixture.]; }}; \
             pagebreak() }}",
            include_str!("typst/theme.typ")
        );
        std::fs::write(root.path().join("main.typ"), source)?;
        let assets = Assets::load().await?;
        let pdf = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            compile(
                root.path().to_owned(),
                assets,
                Arc::new(tokio::sync::Semaphore::new(1)),
            ),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "running headers must not scan every chapter on every page"
            )
        })??;
        assert!(pdf.starts_with(b"%PDF-"));
        Ok(())
    }
}
