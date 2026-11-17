//! One edition of the Commons specification, read page by page.
use {
    super::super::pdf_pages::{OutlineEntry, PageText, PdfPages, TextLine},
    anyhow::{Result, anyhow},
};

/// The bottom band that holds the running footer, in points.
pub const FOOTER_BAND: f64 = 60.0;
/// Clause 8 holds the ontologies.
const ONTOLOGY_CLAUSE: &str = "8 ";

/// Who published an edition, for labels in the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Omg,
    GraphArch,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Self::Omg => "OMG formal/26-06-05",
            Self::GraphArch => "GraphArch edition",
        }
    }
}

/// A numbered figure or table caption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caption {
    pub number: String,
    pub title:  String,
    /// Zero-based page index.
    pub page:   usize,
}

/// The kind of numbered caption.
#[derive(Clone, Copy, Debug)]
pub enum CaptionKind {
    Figure,
    Table,
}

impl CaptionKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Figure => "Figure ",
            Self::Table => "Table ",
        }
    }
}

pub struct Edition {
    pub side:    Side,
    pub pdf:     PdfPages,
    pub pages:   Vec<PageText>, // bounded: the pages of one specification.
    pub outline: Vec<OutlineEntry>,
}

impl Edition {
    /// No accepted edition has more pages than this.
    const PAGE_LIMIT: usize = 2_000;

    pub fn read(side: Side, bytes: Vec<u8>) -> Result<Self> {
        let pdf = PdfPages::read(bytes)?;
        anyhow::ensure!(
            pdf.len() <= Self::PAGE_LIMIT,
            "{} has {} pages, beyond the inspection limit",
            side.label(),
            pdf.len()
        );
        let pages = (0..pdf.len())
            .map(|index| pdf.text(index))
            .collect::<Result<Vec<_>>>()?;
        let outline = pdf.outline()?;
        Ok(Self { side, pdf, pages, outline })
    }

    /// Top-level numbered clauses and annexes, in outline order.
    pub fn clauses(&self) -> Vec<String> {
        self.outline
            .iter()
            .filter(|entry| entry.depth == 0 && is_clause(&entry.title))
            .map(|entry| entry.title.clone())
            .collect()
    }

    /// The outline entries directly below the clause whose title starts
    /// with `number` and a space.
    pub fn subclauses(&self, number: &str) -> Vec<String> {
        let prefix = format!("{number} ");
        let mut inside = false;
        let mut titles = Vec::new();
        for entry in &self.outline {
            if entry.depth == 0 {
                inside = entry.title.starts_with(&prefix);
            } else if inside && entry.depth == 1 {
                titles.push(entry.title.clone());
            }
        }
        titles
    }

    /// The ontology chapter titles of clause 8, without their numbers.
    pub fn ontology_chapters(&self) -> Vec<String> {
        self.subclauses(ONTOLOGY_CLAUSE.trim())
            .into_iter()
            .map(|title| {
                title
                    .split_once(' ')
                    .map_or(title.clone(), |(_, rest)| rest.to_owned())
            })
            .collect()
    }

    /// Every numbered caption of `kind`, in page order.
    pub fn captions(&self, kind: CaptionKind) -> Vec<Caption> {
        let mut captions = Vec::new();
        for (page, text) in self.pages.iter().enumerate() {
            for line in &text.lines {
                let Some(rest) = line.text.strip_prefix(kind.prefix()) else {
                    continue;
                };
                let Some((number, title)) = rest.split_once(':') else {
                    continue;
                };
                let number = number.trim();
                if !number.is_empty() &&
                    number.len() <= 8 &&
                    number.chars().all(|c| {
                        c.is_ascii_digit() || matches!(c, '.' | '-')
                    }) &&
                    number.starts_with(|c: char| c.is_ascii_digit())
                {
                    captions.push(Caption {
                        number: number.to_owned(),
                        title: title.trim().to_owned(),
                        page,
                    });
                }
            }
        }
        captions
    }

    /// The index of the first page with a line satisfying `matches`.
    pub fn page_where(
        &self,
        matches: impl Fn(&TextLine) -> bool,
    ) -> Option<usize> {
        self.pages
            .iter()
            .position(|page| page.lines.iter().any(&matches))
    }

    /// The page on which the heading `title` is set at `size` points or more.
    pub fn heading_page(&self, title: &str, size: f64) -> Result<usize> {
        self.page_where(|line| line.text == title && line.size >= size)
            .ok_or_else(|| {
                anyhow!("{} has no heading {title}", self.side.label())
            })
    }

    /// The running footer of a page, without its page number, and the
    /// number and whether it sits at the left edge.
    pub fn footer(&self, page: usize) -> Option<Footer> {
        let text = &self.pages[page];
        let line = text.footer(FOOTER_BAND).last()?;
        Footer::parse(line, text.width)
    }

    /// Zero-based index of the page that starts clause 1.
    pub fn first_body_page(&self) -> Result<usize> {
        self.heading_page("1 Scope", 16.0)
    }
}

/// A running footer split into its text and page number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footer {
    pub text:        String,
    pub number:      String,
    /// The number is at the left edge rather than the right.
    pub number_left: bool,
}

impl Footer {
    fn parse(line: &TextLine, width: f64) -> Option<Self> {
        let words = line.text.split_whitespace().collect::<Vec<_>>();
        let is_number = |word: &str| {
            word.chars().all(|c| c.is_ascii_digit()) ||
                word.chars().all(|c| "ivxlc".contains(c))
        };
        let (number, rest, number_left) = match words.as_slice() {
            [first, rest @ ..] if is_number(first) && line.x < width / 2.0 => {
                (first, rest, true)
            },
            [rest @ .., last] if is_number(last) => (last, rest, false),
            _ => return None,
        };
        Some(Self {
            text: rest.join(" "),
            number: (*number).to_owned(),
            number_left,
        })
    }
}

/// A numbered clause (`1 Scope`) or an annex (`Annex A: Deliverables`).
fn is_clause(title: &str) -> bool {
    title.starts_with("Annex ") ||
        title
            .split_once(' ')
            .is_some_and(|(number, _)| number.parse::<u8>().is_ok())
}
