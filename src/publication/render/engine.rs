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
}

impl Assets {
    pub async fn load() -> Result<Arc<Self>> {
        tokio::task::spawn_blocking(|| {
            let fonts = Fonts::searcher().include_system_fonts(false).search();
            ensure!(
                !fonts.fonts.is_empty(),
                "Typst requires the embedded font assets"
            );
            Ok(Arc::new(Self {
                library: LazyHash::new(Library::default()),
                book:    LazyHash::new(fonts.book),
                fonts:   fonts.fonts,
            }))
        })
        .await?
    }
}

struct DocumentWorld {
    root:   PathBuf,
    assets: Arc<Assets>,
}

impl World for DocumentWorld {
    fn library(&self) -> &LazyHash<Library> { &self.assets.library }

    fn book(&self) -> &LazyHash<FontBook> { &self.assets.book }

    fn main(&self) -> FileId { FileId::new(None, VirtualPath::new("main.typ")) }

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
        self.assets.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> { None }
}

pub(super) async fn compile(
    root: PathBuf,
    assets: Arc<Assets>,
    slots: Arc<tokio::sync::Semaphore>,
) -> Result<Vec<u8>> {
    let permit = slots.acquire_owned().await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let world = DocumentWorld { root, assets };
        let result = typst::compile::<PagedDocument>(&world);
        let document = result.output.map_err(|diagnostics| {
            anyhow::anyhow!("Typst compilation failed: {diagnostics:?}")
        })?;
        ensure!(
            result.warnings.is_empty(),
            "Typst compilation warnings: {:?}",
            result.warnings
        );
        typst_pdf::pdf(&document, &PdfOptions::default()).map_err(
            |diagnostics| {
                anyhow::anyhow!("PDF generation failed: {diagnostics:?}")
            },
        )
    })
    .await?
}
