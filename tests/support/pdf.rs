//! Reads the accessibility structure and document metadata of generated PDFs.
use {
    anyhow::{Result, anyhow, ensure},
    hayro_syntax::{
        Pdf,
        object::{Dict, Object, Stream, String as PdfString},
    },
};

/// The catalogue, information dictionary, XMP and outline facts that a
/// publication asserts about one generated PDF.
#[derive(Debug)]
pub struct PdfFacts {
    /// `MarkInfo` declares the document as a tagged PDF.
    pub marked:            bool,
    /// The catalogue links a structure tree with at least one child.
    pub structure_tree:    bool,
    /// The catalogue's natural language.
    pub language:          Option<String>,
    /// Viewers display the document title instead of the file name.
    pub display_doc_title: bool,
    pub title:             Option<String>,
    pub author:            Option<String>,
    pub keywords:          Option<String>,
    /// The creation date as year, month and day.
    pub created:           Option<(u16, u8, u8)>,
    /// The XMP metadata packet.
    pub xmp:               String,
    /// Every bookmark title in document order, depth first.
    pub bookmarks:         Vec<String>,
    /// Extracted text, through a ToUnicode map or `ActualText`, contains
    /// U+200B ZERO WIDTH SPACE.
    pub zero_width_text:   bool,
}

impl PdfFacts {
    pub fn read(bytes: Vec<u8>) -> Result<Self> {
        let pdf = Pdf::new(bytes).map_err(|error| {
            anyhow!("generated PDF cannot be parsed: {error:?}")
        })?;
        let catalog: Dict<'_> = pdf
            .xref()
            .get(pdf.xref().root_id())
            .ok_or_else(|| anyhow!("PDF has no catalogue"))?;
        let marked = catalog
            .get::<Dict<'_>>(b"MarkInfo")
            .and_then(|info| info.get::<bool>(b"Marked"))
            .unwrap_or(false);
        let structure_tree = catalog
            .get::<Dict<'_>>(b"StructTreeRoot")
            .is_some_and(|root| root.contains_key(b"K"));
        let language = catalog.get::<PdfString<'_>>(b"Lang").map(text);
        let display_doc_title = catalog
            .get::<Dict<'_>>(b"ViewerPreferences")
            .and_then(|preferences| preferences.get::<bool>(b"DisplayDocTitle"))
            .unwrap_or(false);
        let xmp = catalog
            .get::<Stream<'_>>(b"Metadata")
            .ok_or_else(|| anyhow!("PDF has no XMP metadata"))?
            .decoded()
            .map_err(|error| anyhow!("undecodable XMP metadata: {error:?}"))?;
        let mut bookmarks = Vec::new();
        if let Some(outline) = catalog.get::<Dict<'_>>(b"Outlines") {
            collect_bookmarks(
                outline.get::<Dict<'_>>(b"First"),
                &mut bookmarks,
            )?;
        }
        let zero_width_text = zero_width_text(&pdf)?;
        let info = pdf.metadata();
        let decode = |value: &Option<Vec<u8>>| value.as_deref().map(bytes_text);
        Ok(Self {
            marked,
            structure_tree,
            language,
            display_doc_title,
            title: decode(&info.title),
            author: decode(&info.author),
            keywords: decode(&info.keywords),
            created: info
                .creation_date
                .map(|date| (date.year, date.month, date.day)),
            xmp: String::from_utf8(xmp.into_owned())?,
            bookmarks,
            zero_width_text,
        })
    }

    pub fn of(path: &std::path::Path) -> Result<Self> {
        Self::read(std::fs::read(path)?)
    }

    /// The XMP identification of a PDF/UA part, if declared.
    pub fn pdf_ua_part(&self) -> Option<&str> {
        let (_, rest) = self.xmp.split_once("<pdfuaid:part>")?;
        rest.split_once("</pdfuaid:part>").map(|(part, _)| part)
    }
}

fn collect_bookmarks(
    mut item: Option<Dict<'_>>,
    titles: &mut Vec<String>,
) -> Result<()> {
    while let Some(node) = item {
        ensure!(
            titles.len() < 100_000,
            "bookmark tree exceeds the inspection budget"
        );
        titles.push(
            node.get::<PdfString<'_>>(b"Title")
                .map(text)
                .ok_or_else(|| anyhow!("bookmark without a title"))?,
        );
        collect_bookmarks(node.get::<Dict<'_>>(b"First"), titles)?;
        item = node.get::<Dict<'_>>(b"Next");
    }
    Ok(())
}

/// Whether any ToUnicode map or marked-content `ActualText` in the PDF
/// yields U+200B, which copied, searched and spoken text would then contain.
fn zero_width_text(pdf: &Pdf) -> Result<bool> {
    for (index, object) in pdf.objects().into_iter().enumerate() {
        ensure!(
            index < 1_000_000,
            "PDF exceeds the inspection budget"
        );
        let Object::Stream(stream) = object else {
            continue;
        };
        let Ok(data) = stream.decoded() else {
            // Image and font programs need no text inspection.
            continue;
        };
        let text = String::from_utf8_lossy(&data).to_ascii_uppercase();
        let maps = text.contains("BEGINCMAP") &&
            text.lines().any(|line| {
                // `<glyph> <UTF-16BE text>` bfchar destinations.
                line.split_whitespace()
                    .nth(1)
                    .and_then(|target| target.strip_prefix('<'))
                    .and_then(|target| target.strip_suffix('>'))
                    .is_some_and(utf16_hex_has_zero_width_space)
            });
        let actual = text.match_indices("/ACTUALTEXT<").any(|(at, key)| {
            text[at + key.len()..]
                .split_once('>')
                .is_some_and(|(value, _)| utf16_hex_has_zero_width_space(value))
        });
        if maps || actual {
            return Ok(true);
        }
    }
    Ok(false)
}

fn utf16_hex_has_zero_width_space(hex: &str) -> bool {
    hex.len() % 4 == 0 &&
        hex.as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|unit| unit.eq_ignore_ascii_case(b"200B"))
}

fn text(value: PdfString<'_>) -> String { bytes_text(value.as_bytes()) }

/// Decodes a PDF text string: UTF-16BE with a byte-order mark, or the ASCII
/// subset of PDFDocEncoding written by Typst.
fn bytes_text(bytes: &[u8]) -> String {
    match bytes.strip_prefix(&[0xfe, 0xff]) {
        Some(utf16) => {
            char::decode_utf16(
                utf16
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair)),
            )
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
        },
        None => bytes.iter().map(|&byte| char::from(byte)).collect(),
    }
}
