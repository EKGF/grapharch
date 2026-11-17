//! The terms an edition documents in clause 8, by name, for a term-level
//! comparison that is independent of the layout of each term.
use {
    super::{
        super::pdf_pages::{FontRole, PageText},
        edition::Edition,
    },
    std::collections::BTreeSet,
};

/// A documented term: the local name of a Commons term, or the CURIE of a
/// term outside the Commons namespaces (`time:Instant`, `rdf:langString`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermName(String);

impl TermName {
    /// The term a CURIE names, or `None` when `text` is not one CURIE.
    fn from_curie(text: &str) -> Option<Self> {
        let (prefix, local) = text.split_once(':')?;
        let prefix_ok = !prefix.is_empty() &&
            prefix.starts_with(|c: char| c.is_ascii_lowercase()) &&
            prefix.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
            });
        (prefix_ok && is_identifier(local)).then(|| {
            Self(
                if prefix.starts_with("cmns-") {
                    local.to_owned()
                } else {
                    text.to_owned()
                },
            )
        })
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

/// The names of the terms in clause 8 of an edition: the names in the name
/// column of term tables, rejoined where a long name wraps, and each Commons
/// CURIE set alone on a line in the monospaced font, as identifier rows are.
pub struct TermInventory(pub BTreeSet<TermName>);

impl TermInventory {
    pub fn read(edition: &Edition) -> anyhow::Result<Self> {
        let start = edition.heading_page("8 Commons Ontologies", 16.0)?;
        let end = edition
            .page_where(|line| {
                line.text.starts_with("Annex A") && line.size >= 16.0
            })
            .unwrap_or(edition.pages.len());
        let lines = || {
            edition.pages[start..end]
                .iter()
                .flat_map(|page| page.body(super::edition::FOOTER_BAND))
        };
        let mut names = BTreeSet::new();
        for line in lines() {
            let mono = line
                .font
                .as_ref()
                .is_some_and(|font| font.role() == FontRole::Mono);
            if mono &&
                let Some(curie) = TermName::from_curie(&line.text) &&
                line.text.starts_with("cmns-")
            {
                names.insert(curie);
            }
        }
        // Word sections of the OMG edition indent their tables differently,
        // so each page finds its own name columns; a table that continues
        // onto the next page keeps them.
        let mut cells = NameCells::default();
        let mut columns = Vec::new();
        for page in &edition.pages[start..end] {
            cells.page_break();
            let lines = || page.body(super::edition::FOOTER_BAND);
            let found = name_columns(page);
            if !found.is_empty() {
                columns = found;
            }
            for line in lines() {
                for run in &line.runs {
                    if columns
                        .iter()
                        .any(|column| (run.x - column).abs() <= 1.5) &&
                        line.size <= 9.5 &&
                        run.font
                            .as_ref()
                            .is_none_or(|font| font.role() != FontRole::Mono)
                    {
                        cells.read(line.y, line.size, &run.text);
                    }
                }
            }
        }
        names.extend(cells.names);
        Ok(Self(names))
    }

    /// Names in `self` that `other` lacks.
    pub fn missing_from(&self, other: &Self) -> Vec<TermName> {
        self.0.difference(&other.0).cloned().collect()
    }
}

/// The runs of a term table's name column, top to bottom: each name may be
/// followed by its labels in parentheses, possibly over several lines, and a
/// name too long for the cell wraps onto the next line.
#[derive(Default)]
struct NameCells {
    names:      Vec<TermName>, // bounded: the term names of one clause.
    /// Inside a label in parentheses that has not closed yet.
    open_label: bool,
    /// The last name was set alone on its line, so a wrapped remainder may
    /// continue it.
    lone:       bool,
    /// The baseline of the previous run in the column on this page.
    last:       Option<f64>,
}

impl NameCells {
    /// Forget the previous run: a cell never continues across a page.
    fn page_break(&mut self) {
        self.open_label = false;
        self.lone = false;
        self.last = None;
    }

    fn read(&mut self, y: f64, size: f64, text: &str) {
        if std::env::var("TERM_DEBUG").is_ok_and(|probe| text.contains(&probe))
        {
            eprintln!(
                "DEBUG {y:.1} {size:.1} open={} lone={} last={:?} [{text}]",
                self.open_label, self.lone, self.last
            );
        }
        // Lines within a cell are set closer than 1.3 sizes apart; rows are
        // separated further by their cell padding.
        let same_cell = self
            .last
            .is_some_and(|above| y > above && y - above <= size * 1.3);
        self.last = Some(y);
        if self.open_label && same_cell {
            self.open_label = !text.contains(')');
            self.lone = false;
            return;
        }
        self.open_label = false;
        let mut words = text.split_whitespace();
        let Some(first) = words.next() else { return };
        let single = words.next().is_none();
        let opens = text.contains('(') && !text.contains(')');
        if self.lone &&
            same_cell &&
            first.starts_with(|c: char| c.is_ascii_lowercase()) &&
            is_identifier(first) &&
            let Some(TermName(name)) = self.names.last_mut()
        {
            name.push_str(first);
            self.open_label = opens;
            self.lone = single;
            return;
        }
        // A column header is a header word without a label after it.
        let header = HEADERS.contains(&first) && !text.contains('(');
        let name = TermName::from_curie(first).or_else(|| {
            (is_identifier(first) && !header)
                .then(|| TermName(first.to_owned()))
        });
        self.lone = name.is_some() && single;
        if let Some(name) = name {
            self.names.push(name);
            self.open_label = opens;
        }
    }
}

/// Group rows and column headers of term tables, which are not terms.
const HEADERS: [&str; 9] = [
    "Classes",
    "Properties",
    "Individuals",
    "Datatypes",
    "Name",
    "Annotations",
    "Term",
    "Reference",
    "RDF",
];

/// The left edges of the name columns of a page's term tables, of which a
/// page can hold several, each indented differently: where a table-size run
/// opens with a name followed by its label in parentheses. Labels spell out
/// their names, so the label starts with the name's first three letters,
/// which prose in parentheses does not. A row of a term without a label
/// opens with its CURIE alone.
fn name_columns(page: &PageText) -> Vec<f64> {
    let mut edges = std::collections::BTreeSet::<i64>::new();
    let runs = page
        .body(super::edition::FOOTER_BAND)
        .filter(|line| (8.0..=9.5).contains(&line.size))
        .flat_map(|line| &line.runs)
        .filter(|run| {
            run.font
                .as_ref()
                .is_none_or(|font| font.role() != FontRole::Mono)
        });
    for line in page
        .body(super::edition::FOOTER_BAND)
        .filter(|line| (8.0..=9.5).contains(&line.size) && line.runs.len() >= 2)
    {
        // A name cell without a label: a lone CURIE that opens a table row,
        // as the rows of terms outside the Commons namespaces are.
        let first = &line.runs[0];
        if !first.text.contains(' ') &&
            TermName::from_curie(&first.text).is_some() &&
            first
                .font
                .as_ref()
                .is_none_or(|font| font.role() != FontRole::Mono)
        {
            edges.insert(first.x.round() as i64);
        }
    }
    for run in runs {
        let mut words = run.text.split_whitespace();
        let (Some(name), Some(label)) = (words.next(), words.next()) else {
            continue;
        };
        let local = name.rsplit_once(':').map_or(name, |(_, local)| local);
        let spelled = |text: &str| {
            text.chars()
                .take(3)
                .collect::<String>()
                .to_ascii_lowercase()
        };
        if (is_identifier(name) || TermName::from_curie(name).is_some()) &&
            label.strip_prefix('(').is_some_and(|label| {
                label.len() >= 3 && spelled(label) == spelled(local)
            })
        {
            edges.insert(run.x.round() as i64);
        }
    }
    edges.into_iter().map(|edge| edge as f64).collect()
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() &&
        text.starts_with(|c: char| c.is_ascii_alphabetic()) &&
        text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
