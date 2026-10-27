//! The audited synchronous Typst boundary. All functions execute on bounded
//! blocking workers; no source graph or corpus is hydrated into this world.

use {
    anyhow::{Result, ensure},
    std::{path::PathBuf, sync::Arc},
    typst::{
        Library,
        World,
        diag::FileError,
        foundations::{Bytes, Datetime},
        layout::PagedDocument,
        syntax::{FileId, Source, VirtualPath},
        text::{Font, FontBook},
        utils::LazyHash,
    },
    typst_kit::fonts::{FontSlot, Fonts},
    typst_pdf::PdfOptions,
};

pub(super) struct Assets {
    library: LazyHash<Library>,
    book:    LazyHash<FontBook>,
    // bounded: fixed embedded font assets, independent of graph/document size.
    fonts:   Vec<FontSlot>,
    sans:    Vec<Font>,
}

impl Assets {
    pub async fn load() -> Result<Arc<Self>> {
        tokio::task::spawn_blocking(|| {
            let fonts = Fonts::searcher().include_system_fonts(false).search();
            ensure!(
                !fonts.fonts.is_empty(),
                "Typst requires the embedded font assets"
            );
            let sans = [
                include_bytes!("fonts/LiberationSans-Regular.ttf").as_slice(),
                include_bytes!("fonts/LiberationSans-Bold.ttf").as_slice(),
                include_bytes!("fonts/LiberationSans-Italic.ttf").as_slice(),
                include_bytes!("fonts/LiberationSans-BoldItalic.ttf")
                    .as_slice(),
            ]
            .into_iter()
            .map(|bytes| {
                Font::new(Bytes::new(bytes), 0)
                    .ok_or_else(|| anyhow::anyhow!("invalid embedded PDF font"))
            })
            .collect::<Result<Vec<_>>>()?;
            let mut book = fonts.book;
            for font in &sans {
                book.push(font.info().clone());
            }
            Ok(Arc::new(Self {
                library: LazyHash::new(Library::default()),
                book: LazyHash::new(book),
                fonts: fonts.fonts,
                sans,
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

impl World for DocumentWorld {
    fn library(&self) -> &LazyHash<Library> { &self.assets.library }

    fn book(&self) -> &LazyHash<FontBook> { &self.assets.book }

    fn main(&self) -> FileId { self.main }

    fn source(&self, id: FileId) -> std::result::Result<Source, FileError> {
        let path = self.root.join(id.vpath().as_rootless_path());
        if id.package().is_some() {
            return Err(FileError::NotFound(path));
        }
        std::fs::read_to_string(&path)
            .map(|source| Source::new(id, source))
            .map_err(|error| FileError::from_io(error, &path))
    }

    fn file(&self, id: FileId) -> std::result::Result<Bytes, FileError> {
        let path = self.root.join(id.vpath().as_rootless_path());
        if id.package().is_some() {
            return Err(FileError::NotFound(path));
        }
        std::fs::read(&path)
            .map(Bytes::new)
            .map_err(|error| FileError::from_io(error, &path))
    }

    fn font(&self, index: usize) -> Option<Font> {
        if index < self.assets.fonts.len() {
            self.assets.fonts.get(index)?.get()
        } else {
            self.assets
                .sans
                .get(index - self.assets.fonts.len())
                .cloned()
        }
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> { None }
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
            main: FileId::new(None, VirtualPath::new(main)),
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
            pages = document.pages.len(),
            "Typst document laid out"
        );
        let pdf_started = std::time::Instant::now();
        let pdf = typst_pdf::pdf(&document, &PdfOptions::default()).map_err(
            |diagnostics| {
                anyhow::anyhow!("PDF generation failed: {diagnostics:?}")
            },
        )?;
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
