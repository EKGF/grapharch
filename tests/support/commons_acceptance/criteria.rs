//! Acceptance criteria: each compares one structural or typographic fact of
//! the GraphArch edition with the OMG formal specification.
use {
    super::{
        super::pdf_pages::{FontRole, TextLine},
        criterion::Criterion,
        edition::{CaptionKind, Edition},
        style::{Style, Styles},
        terms::TermInventory,
    },
    anyhow::Result,
    std::fmt,
};

/// The outcome of one criterion.
#[derive(Debug)]
pub struct Finding {
    pub criterion: Criterion,
    pub omg:       String,
    pub ours:      String,
    pub met:       bool,
    /// Lines explaining a difference, such as missing items.
    pub detail:    Vec<String>,
}

impl Finding {
    fn compare<T: PartialEq + fmt::Debug>(
        criterion: Criterion,
        omg: T,
        ours: T,
    ) -> Self {
        Self {
            criterion,
            met: omg == ours,
            omg: format!("{omg:?}"),
            ours: format!("{ours:?}"),
            detail: Vec::new(),
        }
    }

    fn missing(
        criterion: Criterion,
        omg: usize,
        ours: usize,
        missing: Vec<String>,
    ) -> Self {
        Self {
            criterion,
            omg: format!("{omg}"),
            ours: format!("{ours}"),
            met: missing.is_empty(),
            detail: missing
                .into_iter()
                .map(|item| format!("missing: {item}"))
                .collect(),
        }
    }

    fn within(
        criterion: Criterion,
        omg: f64,
        ours: f64,
        tolerance: f64,
    ) -> Self {
        Self {
            criterion,
            omg: format!("{omg:.1} pt"),
            ours: format!("{ours:.1} pt"),
            met: (omg - ours).abs() <= tolerance,
            detail: Vec::new(),
        }
    }
}

/// Evaluates every criterion for the OMG `omg` and GraphArch `ours` editions.
pub struct Acceptance<'a> {
    pub omg:  &'a Edition,
    pub ours: &'a Edition,
}

impl Acceptance<'_> {
    pub fn findings(&self) -> Result<Vec<Finding>> {
        Criterion::ALL
            .iter()
            .map(|criterion| self.evaluate(*criterion))
            .collect()
    }

    fn evaluate(&self, criterion: Criterion) -> Result<Finding> {
        let (omg, ours) = (self.omg, self.ours);
        Ok(match criterion {
            Criterion::CoverText => {
                let lines = |edition: &Edition| texts(&edition.pages[0].lines);
                let ours_lines = lines(ours);
                let theirs = lines(omg);
                let missing = theirs
                    .iter()
                    .filter(|line| !ours_lines.contains(line))
                    .cloned()
                    .collect();
                Finding::missing(
                    criterion,
                    theirs.len(),
                    ours_lines.len(),
                    missing,
                )
            },
            Criterion::CoverTypography => {
                let mut finding = Finding::missing(criterion, 0, 0, Vec::new());
                for line in &omg.pages[0].lines {
                    let theirs = Style::of(line);
                    let ours = ours.pages[0]
                        .lines
                        .iter()
                        .find(|candidate| candidate.text == line.text)
                        .map(Style::of);
                    if ours != Some(theirs) {
                        finding.met = false;
                        finding.detail.push(format!(
                            "{}: OMG {theirs}, ours {ours:?}",
                            line.text
                        ));
                    }
                }
                finding.omg = format!("{} lines", omg.pages[0].lines.len());
                finding.ours = format!("{} lines", ours.pages[0].lines.len());
                finding
            },
            Criterion::PageSize => {
                Finding::compare(criterion, page_size(omg), page_size(ours))
            },
            Criterion::TextBlockLeftEdge => {
                Finding::within(
                    criterion,
                    Styles::prose(omg)?.left_edge(),
                    Styles::prose(ours)?.left_edge(),
                    1.5,
                )
            },
            Criterion::FooterBaseline => {
                let (omg, ours) = (
                    Styles::prose(omg)?.footer_heights(),
                    Styles::prose(ours)?.footer_heights(),
                );
                Finding {
                    criterion,
                    omg: format!(
                        "{:.1}–{:.1} pt above the bottom",
                        omg.0, omg.1
                    ),
                    ours: format!(
                        "{:.1}–{:.1} pt above the bottom",
                        ours.0, ours.1
                    ),
                    met: omg.0 <= ours.0 && ours.1 <= omg.1,
                    detail: Vec::new(),
                }
            },
            Criterion::Clauses => {
                Finding::compare(criterion, omg.clauses(), ours.clauses())
            },
            Criterion::Subclauses => {
                Finding::compare(
                    criterion,
                    (1..=7)
                        .map(|n| omg.subclauses(&n.to_string()))
                        .collect::<Vec<_>>(),
                    (1..=7)
                        .map(|n| ours.subclauses(&n.to_string()))
                        .collect::<Vec<_>>(),
                )
            },
            Criterion::OntologyChapters => {
                Finding::compare(
                    criterion,
                    omg.ontology_chapters(),
                    ours.ontology_chapters(),
                )
            },
            Criterion::ContentsTitle => {
                Finding::compare(
                    criterion,
                    contents_title(omg),
                    contents_title(ours),
                )
            },
            Criterion::FigureCaptions => {
                captions(criterion, CaptionKind::Figure, omg, ours)
            },
            Criterion::TableCaptions => {
                captions(criterion, CaptionKind::Table, omg, ours)
            },
            Criterion::BodyTypography => {
                Finding::compare(
                    criterion,
                    Styles::prose(omg)?.dominant(),
                    Styles::prose(ours)?.dominant(),
                )
            },
            Criterion::HeadingTypography => {
                Finding::compare(
                    criterion,
                    heading_styles(omg),
                    heading_styles(ours),
                )
            },
            Criterion::FontFamilies => {
                let families = Styles::families(ours);
                let others: Vec<_> = families
                    .iter()
                    .filter(|family| family.role() == FontRole::Other)
                    .map(ToString::to_string)
                    .collect();
                let mut finding =
                    Finding::missing(criterion, 0, families.len(), Vec::new());
                finding.omg = Styles::families(omg)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                finding.ours = families
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                finding.met = others.is_empty();
                finding.detail = others
                    .into_iter()
                    .map(|family| format!("other font: {family}"))
                    .collect();
                finding
            },
            Criterion::RunningFooter => {
                let footers = |edition: &Edition| -> Result<_> {
                    let first = edition.first_body_page()?;
                    Ok([edition.footer(first), edition.footer(first + 1)])
                };
                Finding::compare(criterion, footers(omg)?, footers(ours)?)
            },
            Criterion::FrontMatterPages => {
                Finding::compare(
                    criterion,
                    omg.first_body_page()?,
                    ours.first_body_page()?,
                )
            },
            Criterion::PageCount => {
                let (theirs, pages) = (omg.pages.len(), ours.pages.len());
                Finding {
                    criterion,
                    omg: theirs.to_string(),
                    ours: pages.to_string(),
                    met: pages.abs_diff(theirs) * 10 <= theirs,
                    detail: Vec::new(),
                }
            },
            Criterion::Terms => {
                let (theirs, documented) = (
                    TermInventory::read(omg)?,
                    TermInventory::read(ours)?,
                );
                let missing = theirs.missing_from(&documented);
                let mut finding = Finding::missing(
                    criterion,
                    theirs.0.len(),
                    documented.0.len(),
                    missing
                        .iter()
                        .map(|name| name.as_str().to_owned())
                        .collect(),
                );
                finding.detail.extend(
                    documented
                        .missing_from(&theirs)
                        .iter()
                        .map(|name| format!("only in ours: {}", name.as_str())),
                );
                finding
            },
        })
    }
}

fn texts(lines: &[TextLine]) -> Vec<String> {
    lines.iter().map(|line| line.text.clone()).collect()
}

fn page_size(edition: &Edition) -> (i64, i64) {
    let mut sizes = std::collections::BTreeMap::<(i64, i64), usize>::new();
    for page in &edition.pages {
        *sizes
            .entry((
                page.width.round() as i64,
                page.height.round() as i64,
            ))
            .or_default() += 1;
    }
    sizes
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or((0, 0), |(size, _)| size)
}

/// The style of the contents title and whether it is centred.
fn contents_title(edition: &Edition) -> Option<(Style, bool)> {
    edition.pages.iter().find_map(|page| {
        page.lines
            .iter()
            .find(|line| line.text == "Table of Contents")
            .map(|line| {
                let centre = (line.x + line.right) / 2.0;
                (
                    Style::of(line),
                    (centre - page.width / 2.0).abs() < 6.0,
                )
            })
    })
}

/// The styles of the clause 1 and subclause 1.1 headings.
fn heading_styles(edition: &Edition) -> [Option<Style>; 2] {
    ["1 Scope", "1.1 Introduction"].map(|title| {
        edition
            .pages
            .iter()
            .flat_map(|page| &page.lines)
            .find(|line| line.text == title && line.size >= 12.0)
            .map(Style::of)
    })
}

fn captions(
    criterion: Criterion,
    kind: CaptionKind,
    omg: &Edition,
    ours: &Edition,
) -> Finding {
    let ours_captions = ours.captions(kind);
    let theirs = omg.captions(kind);
    let missing = theirs
        .iter()
        .filter(|caption| {
            !ours_captions.iter().any(|candidate| {
                candidate.number == caption.number &&
                    candidate.title == caption.title
            })
        })
        .map(|caption| {
            format!(
                "{} {}: {}",
                kind_label(kind),
                caption.number,
                caption.title
            )
        })
        .collect();
    Finding::missing(
        criterion,
        theirs.len(),
        ours_captions.len(),
        missing,
    )
}

fn kind_label(kind: CaptionKind) -> &'static str {
    match kind {
        CaptionKind::Figure => "Figure",
        CaptionKind::Table => "Table",
    }
}
