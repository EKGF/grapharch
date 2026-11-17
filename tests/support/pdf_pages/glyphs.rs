//! A drawing device that records where each glyph is painted, with its text,
//! size and font, and discards everything else.
use {
    super::font::FontFamily,
    hayro::{
        hayro_interpret::{
            BlendMode,
            ClipPath,
            Device,
            GlyphDrawMode,
            Image,
            Paint,
            PathDrawMode,
            SoftMask,
            font::Glyph,
            hayro_cmap::BfString,
        },
        vello_cpu::kurbo::{Affine, BezPath, Point},
    },
};

/// A painted glyph, in page points with the origin at the top left.
#[derive(Clone, Debug)]
pub(super) struct PlacedGlyph {
    pub x:       f64,
    pub y:       f64,
    pub size:    f64,
    pub advance: f64,
    pub text:    String,
    pub font:    Option<FontFamily>,
}

/// The glyphs of one page, bounded by `limit`.
pub(super) struct GlyphRecorder {
    pub glyphs: Vec<PlacedGlyph>, // bounded: `limit` glyphs of one page.
    limit:      usize,
}

impl GlyphRecorder {
    pub fn new(limit: usize) -> Self { Self { glyphs: Vec::new(), limit } }
}

impl<'a> Device<'a> for GlyphRecorder {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}

    fn set_blend_mode(&mut self, _: BlendMode) {}

    fn draw_path(
        &mut self,
        _: &BezPath,
        _: Affine,
        _: &Paint<'a>,
        _: &PathDrawMode,
    ) {
    }

    fn push_clip_path(&mut self, _: &ClipPath) {}

    fn push_transparency_group(
        &mut self,
        _: f32,
        _: Option<SoftMask<'a>>,
        _: BlendMode,
    ) {
    }

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        if self.glyphs.len() >= self.limit {
            return;
        }
        let Some(text) = glyph.as_unicode().map(|text| {
            match text {
                BfString::Char(character) => character.to_string(),
                BfString::String(text) => text,
            }
        }) else {
            return;
        };
        // Outlines are in units of a 1000-unit em.
        let placed = transform * glyph_transform;
        let origin = placed * Point::ORIGIN;
        let [_, _, c, d, _, _] = placed.as_coeffs();
        let (font, advance) = match glyph {
            Glyph::Outline(outline) => {
                (
                    outline
                        .font_data()
                        .and_then(|data| data.postscript_name)
                        .map(|name| FontFamily::from_postscript(&name)),
                    outline.advance_width().map_or(0.0, f64::from),
                )
            },
            Glyph::Type3(_) => (None, 0.0),
        };
        let [a, b, ..] = placed.as_coeffs();
        self.glyphs.push(PlacedGlyph {
            x: origin.x,
            y: origin.y,
            size: c.hypot(d) * 1000.0,
            advance: advance * a.hypot(b),
            text,
            font,
        });
    }

    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}

    fn pop_clip_path(&mut self) {}

    fn pop_transparency_group(&mut self) {}
}
