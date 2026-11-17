//! Page-level inspection of PDFs: positioned text lines with their fonts and
//! sizes, page geometry, the outline, and rasterized page images.
mod document;
mod font;
mod glyphs;
mod lines;
mod raster;

pub use {
    document::{OutlineEntry, PdfPages},
    font::{FontFamily, FontRole},
    lines::{PageText, TextLine},
    raster::PageImage,
};
