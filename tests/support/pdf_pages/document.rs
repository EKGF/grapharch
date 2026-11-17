//! A parsed PDF whose pages can be read as text or rendered as images.
use {
    super::{glyphs::GlyphRecorder, lines::PageText, raster::PageImage},
    anyhow::{Result, anyhow, ensure},
    hayro::{
        RenderCache,
        RenderSettings,
        hayro_interpret::{
            Context,
            InterpreterCache,
            InterpreterSettings,
            TransformExt,
            hayro_syntax::{
                Pdf,
                object::{Dict, String as PdfString},
            },
            interpret_page,
        },
        vello_cpu::{color::palette::css::WHITE, kurbo::Rect},
    },
};

/// No inspected page has more glyphs than this.
const GLYPH_LIMIT: usize = 200_000;
/// No inspected outline has more entries than this.
const OUTLINE_LIMIT: usize = 100_000;

/// One bookmark, with its nesting depth from zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineEntry {
    pub depth: usize,
    pub title: String,
}

pub struct PdfPages {
    pdf: Pdf,
}

impl PdfPages {
    pub fn read(bytes: Vec<u8>) -> Result<Self> {
        let pdf = Pdf::new(bytes)
            .map_err(|error| anyhow!("PDF cannot be parsed: {error:?}"))?;
        Ok(Self { pdf })
    }

    pub fn len(&self) -> usize { self.pdf.pages().len() }

    /// The positioned text of the page at zero-based `index`.
    pub fn text(&self, index: usize) -> Result<PageText> {
        let page = self
            .pdf
            .pages()
            .get(index)
            .ok_or_else(|| anyhow!("no page {index}"))?;
        let (width, height) = page.render_dimensions();
        let cache = InterpreterCache::new();
        let mut context = Context::new(
            page.initial_transform(true).to_kurbo(),
            Rect::new(0.0, 0.0, width.into(), height.into()),
            &cache,
            page.xref(),
            InterpreterSettings::default(),
        );
        let mut recorder = GlyphRecorder::new(GLYPH_LIMIT);
        interpret_page(page, &mut context, &mut recorder);
        Ok(PageText::new(
            width.into(),
            height.into(),
            recorder.glyphs,
        ))
    }

    /// The page at zero-based `index`, rendered at `dpi`.
    pub fn render(&self, index: usize, dpi: f32) -> Result<PageImage> {
        let page = self
            .pdf
            .pages()
            .get(index)
            .ok_or_else(|| anyhow!("no page {index}"))?;
        let scale = dpi / 72.0;
        let pixmap = hayro::render(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
                ..Default::default()
            },
        );
        PageImage::from_pixmap(pixmap)
    }

    /// Every bookmark in document order, depth first.
    pub fn outline(&self) -> Result<Vec<OutlineEntry>> {
        let mut entries = Vec::new();
        if let Some(outline) = self
            .catalog()
            .and_then(|catalog| catalog.get::<Dict<'_>>(b"Outlines"))
        {
            collect(outline.get::<Dict<'_>>(b"First"), 0, &mut entries)?;
        }
        Ok(entries)
    }

    fn catalog(&self) -> Option<Dict<'_>> {
        self.pdf.xref().get(self.pdf.xref().root_id())
    }
}

fn collect(
    mut item: Option<Dict<'_>>,
    depth: usize,
    entries: &mut Vec<OutlineEntry>,
) -> Result<()> {
    while let Some(node) = item {
        ensure!(
            entries.len() < OUTLINE_LIMIT && depth < 32,
            "outline exceeds the inspection budget"
        );
        let title = node
            .get::<PdfString<'_>>(b"Title")
            .map(|title| decode(title.as_bytes()))
            .ok_or_else(|| anyhow!("bookmark without a title"))?;
        entries.push(OutlineEntry { depth, title });
        collect(node.get::<Dict<'_>>(b"First"), depth + 1, entries)?;
        item = node.get::<Dict<'_>>(b"Next");
    }
    Ok(())
}

/// A PDF text string: UTF-16BE with a byte-order mark, or else
/// PDFDocEncoding.
fn decode(bytes: &[u8]) -> String {
    match bytes.strip_prefix(&[0xFE, 0xFF]) {
        Some(utf16) => {
            char::decode_utf16(
                utf16
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair)),
            )
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
        },
        None => bytes.iter().map(|&byte| pdf_doc_char(byte)).collect(),
    }
}

/// PDFDocEncoding: Latin-1 except for the typographic characters at
/// 0x80–0x9F (ISO 32000-2, Annex D).
fn pdf_doc_char(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“',
        '”', '‘', '’', '‚', '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł',
        'œ', 'š', 'ž', '\u{fffd}',
    ];
    match byte {
        0x80..=0x9F => HIGH[usize::from(byte - 0x80)],
        0xA0 => '€',
        _ => char::from(byte),
    }
}

#[test]
fn pdf_doc_encoding_maps_typographic_quotes() {
    assert_eq!(
        decode(b"7.1 \x8dAbout\x8e the"),
        "7.1 “About” the"
    );
    assert_eq!(decode(b"\xfe\xff\x00A\x20\x14"), "A—");
}
