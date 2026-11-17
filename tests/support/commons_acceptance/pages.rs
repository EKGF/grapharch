//! Corresponding pages of both editions, rendered side by side.
use {
    super::edition::Edition,
    crate::pdf_pages::PageImage,
    anyhow::Result,
    std::path::{Path, PathBuf},
};

/// A page that both editions have, located by its content.
#[derive(Clone, Copy, Debug)]
pub enum PageAnchor {
    Cover,
    Notices,
    Contents,
    Scope,
    Notation,
    Namespaces,
    FirstOntology,
    AnnexA,
    AnnexB,
    LastPage,
}

impl PageAnchor {
    pub const ALL: [Self; 10] = [
        Self::Cover,
        Self::Notices,
        Self::Contents,
        Self::Scope,
        Self::Notation,
        Self::Namespaces,
        Self::FirstOntology,
        Self::AnnexA,
        Self::AnnexB,
        Self::LastPage,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Notices => "notices",
            Self::Contents => "contents",
            Self::Scope => "clause-1-scope",
            Self::Notation => "clause-6-5-notation",
            Self::Namespaces => "clause-7-2-namespaces",
            Self::FirstOntology => "clause-8-1-first-ontology",
            Self::AnnexA => "annex-a",
            Self::AnnexB => "annex-b",
            Self::LastPage => "last-page",
        }
    }

    /// The zero-based page of this anchor in `edition`.
    pub fn locate(self, edition: &Edition) -> Result<usize> {
        let heading = |prefix: &str, size: f64| {
            edition
                .page_where(|line| {
                    line.text.starts_with(prefix) && line.size >= size
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{} has no {prefix} heading",
                        edition.side.label()
                    )
                })
        };
        match self {
            Self::Cover => Ok(0),
            Self::Notices => heading("USE OF SPECIFICATION", 10.5),
            Self::Contents => heading("Table of Contents", 16.0),
            Self::Scope => edition.first_body_page(),
            Self::Notation => heading("6.5 Notation", 13.0),
            Self::Namespaces => heading("7.2 Namespace Definitions", 13.0),
            Self::FirstOntology => heading("8.1 ", 13.0),
            Self::AnnexA => heading("Annex A", 16.0),
            Self::AnnexB => heading("Annex B", 16.0),
            Self::LastPage => Ok(edition.pages.len() - 1),
        }
    }
}

/// One rendered pair of corresponding pages.
pub struct PagePair {
    pub anchor:   PageAnchor,
    /// One-based page numbers in the OMG and GraphArch editions.
    pub omg:      usize,
    pub ours:     usize,
    /// See [`PageImage::layout_distance`].
    pub distance: f64,
    pub image:    PathBuf,
}

impl PagePair {
    const DPI: f32 = 80.0;
    /// OMG's page framed in blue on the left, ours in green on the right.
    const OMG_FRAME: [u8; 3] = [30, 90, 200];
    const OUR_FRAME: [u8; 3] = [30, 160, 70];

    pub fn render(
        anchor: PageAnchor,
        omg: &Edition,
        ours: &Edition,
        directory: &Path,
        index: usize,
    ) -> Result<Self> {
        let (left, right) = (anchor.locate(omg)?, anchor.locate(ours)?);
        let (left_image, right_image) = (
            omg.pdf.render(left, Self::DPI)?,
            ours.pdf.render(right, Self::DPI)?,
        );
        let image =
            directory.join(format!("{:02}-{}.png", index + 1, anchor.slug()));
        PageImage::side_by_side(
            &left_image,
            &right_image,
            Self::OMG_FRAME,
            Self::OUR_FRAME,
        )
        .save(&image)?;
        Ok(Self {
            anchor,
            omg: left + 1,
            ours: right + 1,
            distance: left_image.layout_distance(&right_image),
            image,
        })
    }
}
