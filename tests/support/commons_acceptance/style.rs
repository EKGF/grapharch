//! Typographic measurements over the body pages of an edition.
use {
    super::{
        super::pdf_pages::{FontFamily, FontRole, TextLine},
        edition::{Edition, FOOTER_BAND},
    },
    anyhow::Result,
    std::{collections::BTreeMap, fmt},
};

/// A font role at a size, rounded to half a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Style {
    pub role:    FontRole,
    half_points: u32,
}

impl Style {
    pub fn of(line: &TextLine) -> Self {
        Self {
            role:        line
                .font
                .as_ref()
                .map_or(FontRole::Other, FontFamily::role),
            half_points: (line.size * 2.0).round() as u32,
        }
    }
}

impl fmt::Display for Style {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} {} pt",
            self.role,
            f64::from(self.half_points) / 2.0
        )
    }
}

/// The lines of the authored clauses 1 to 7, whose text both editions set
/// as running prose; clause 8 is dominated by term tables.
pub struct Styles<'a> {
    edition: &'a Edition,
    first:   usize,
    end:     usize,
}

impl<'a> Styles<'a> {
    pub fn prose(edition: &'a Edition) -> Result<Self> {
        Ok(Self {
            edition,
            first: edition.first_body_page()?,
            end: edition.heading_page("8 Commons Ontologies", 16.0)?,
        })
    }

    fn pages(&self) -> &'a [super::super::pdf_pages::PageText] {
        &self.edition.pages[self.first..self.end]
    }

    fn lines(&self) -> impl Iterator<Item = &'a TextLine> {
        self.pages().iter().flat_map(|page| page.body(FOOTER_BAND))
    }

    /// The style that sets the most characters of body text.
    pub fn dominant(&self) -> Option<Style> {
        let mut weights = BTreeMap::<Style, usize>::new();
        for line in self.lines() {
            *weights.entry(Style::of(line)).or_default() +=
                line.text.chars().count();
        }
        weights
            .into_iter()
            .max_by_key(|(_, weight)| *weight)
            .map(|(style, _)| style)
    }

    /// The most frequent left edge of lines in the dominant style.
    pub fn left_edge(&self) -> f64 {
        let dominant = self.dominant();
        mode(
            self.lines()
                .filter(|line| Some(Style::of(line)) == dominant)
                .map(|line| line.x),
        )
    }

    /// The lowest and highest distance of a running footer's baseline from
    /// the bottom of its page, in points.
    pub fn footer_heights(&self) -> (f64, f64) {
        self.pages()
            .iter()
            .filter_map(|page| {
                page.footer(FOOTER_BAND)
                    .last()
                    .map(|line| page.height - line.y)
            })
            .fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(low, high), height| (low.min(height), high.max(height)),
            )
    }

    /// Every font family the edition uses, on any page.
    pub fn families(edition: &Edition) -> Vec<FontFamily> {
        let mut families = std::collections::BTreeSet::new();
        for page in &edition.pages {
            families.extend(page.fonts.iter().cloned());
        }
        families.into_iter().collect()
    }
}

/// The most frequent value, at a resolution of a tenth of a point.
fn mode(values: impl Iterator<Item = f64>) -> f64 {
    let mut counts = BTreeMap::<i64, usize>::new();
    for value in values {
        *counts.entry((value * 10.0).round() as i64).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or(0.0, |(value, _)| value as f64 / 10.0)
}
