//! Rendered page images, their side-by-side composition and a coarse
//! layout distance between two pages.
use {
    anyhow::{Result, anyhow},
    hayro::vello_cpu::Pixmap,
    image::{GrayImage, Rgba, RgbaImage, imageops},
};

/// One rendered page.
pub struct PageImage(RgbaImage);

impl PageImage {
    pub(super) fn from_pixmap(pixmap: Pixmap) -> Result<Self> {
        let (width, height) = (pixmap.width().into(), pixmap.height().into());
        let bytes = pixmap
            .take_unpremultiplied()
            .into_iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
            .collect();
        RgbaImage::from_raw(width, height, bytes)
            .map(Self)
            .ok_or_else(|| anyhow!("pixmap size does not match its data"))
    }

    /// `left` and `right` next to each other on a grey canvas, each framed
    /// in `left_frame` and `right_frame`, top aligned.
    pub fn side_by_side(
        left: &Self,
        right: &Self,
        left_frame: [u8; 3],
        right_frame: [u8; 3],
    ) -> Self {
        const GAP: u32 = 24;
        const FRAME: u32 = 4;
        let width = left.0.width() + right.0.width() + GAP * 3 + FRAME * 4;
        let height =
            left.0.height().max(right.0.height()) + GAP * 2 + FRAME * 2;
        let mut canvas =
            RgbaImage::from_pixel(width, height, Rgba([200, 200, 200, 255]));
        let mut x = GAP;
        for (page, [r, g, b]) in [(left, left_frame), (right, right_frame)] {
            let frame = RgbaImage::from_pixel(
                page.0.width() + FRAME * 2,
                page.0.height() + FRAME * 2,
                Rgba([r, g, b, 255]),
            );
            imageops::overlay(&mut canvas, &frame, x.into(), GAP.into());
            imageops::overlay(
                &mut canvas,
                &page.0,
                (x + FRAME).into(),
                (GAP + FRAME).into(),
            );
            x += page.0.width() + FRAME * 2 + GAP;
        }
        Self(canvas)
    }

    /// The mean absolute difference, from 0 (identical) to 1, between the
    /// ink density of both pages on a 32 × 32 grid. It compares layout, not
    /// wording.
    pub fn layout_distance(&self, other: &Self) -> f64 {
        const GRID: u32 = 32;
        let density = |image: &Self| -> GrayImage {
            imageops::resize(
                &imageops::grayscale(&image.0),
                GRID,
                GRID,
                imageops::FilterType::Triangle,
            )
        };
        let (left, right) = (density(self), density(other));
        let total: f64 = left
            .pixels()
            .zip(right.pixels())
            .map(|(a, b)| f64::from(a.0[0].abs_diff(b.0[0])) / 255.0)
            .sum();
        total / f64::from(GRID * GRID)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.0.save(path)?;
        Ok(())
    }
}
