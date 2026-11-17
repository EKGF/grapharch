//! The acceptance criteria and what each requires.
use std::fmt;

/// One acceptance criterion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Criterion {
    CoverText,
    CoverTypography,
    PageSize,
    TextBlockLeftEdge,
    FooterBaseline,
    Clauses,
    Subclauses,
    OntologyChapters,
    ContentsTitle,
    FigureCaptions,
    TableCaptions,
    BodyTypography,
    HeadingTypography,
    FontFamilies,
    RunningFooter,
    FrontMatterPages,
    PageCount,
    Terms,
}

impl Criterion {
    pub const ALL: [Self; 18] = [
        Self::CoverText,
        Self::CoverTypography,
        Self::PageSize,
        Self::TextBlockLeftEdge,
        Self::FooterBaseline,
        Self::Clauses,
        Self::Subclauses,
        Self::OntologyChapters,
        Self::ContentsTitle,
        Self::FigureCaptions,
        Self::TableCaptions,
        Self::BodyTypography,
        Self::HeadingTypography,
        Self::FontFamilies,
        Self::RunningFooter,
        Self::FrontMatterPages,
        Self::PageCount,
        Self::Terms,
    ];

    /// What the criterion requires of the GraphArch edition.
    pub fn requirement(self) -> &'static str {
        match self {
            Self::CoverText => "every OMG cover line appears on the cover",
            Self::CoverTypography => {
                "each OMG cover line has OMG's font role and size"
            },
            Self::PageSize => "the dominant page size equals OMG's",
            Self::TextBlockLeftEdge => {
                "prose in clauses 1 to 7 starts within 1.5 pt of OMG's left \
                 edge"
            },
            Self::FooterBaseline => {
                "footer baselines in clauses 1 to 7 lie within the range of \
                 OMG's, whose Word sections differ"
            },
            Self::Clauses => {
                "the numbered clauses and annexes equal OMG's, in order"
            },
            Self::Subclauses => "the subclauses of clauses 1 to 7 equal OMG's",
            Self::OntologyChapters => {
                "the ontology chapters of clause 8 equal OMG's, in order"
            },
            Self::ContentsTitle => {
                "the contents start with OMG's centred title and type"
            },
            Self::FigureCaptions => {
                "every OMG figure caption appears with its number"
            },
            Self::TableCaptions => {
                "every OMG table caption appears with its number"
            },
            Self::BodyTypography => {
                "prose in clauses 1 to 7 has OMG's font role and size"
            },
            Self::HeadingTypography => {
                "clause and subclause headings have OMG's font role and size"
            },
            Self::FontFamilies => {
                "letters and digits are set only in metric twins of OMG's \
                 serif, sans and mono fonts"
            },
            Self::RunningFooter => {
                "the footers of pages 1 and 2 equal OMG's, number on the outer \
                 edge"
            },
            Self::FrontMatterPages => {
                "clause 1 starts after as many front-matter pages as OMG's"
            },
            Self::PageCount => {
                "the page count is within 10% of OMG's, as metric-twin fonts \
                 and table layout break lines differently"
            },
            Self::Terms => "every term OMG documents in clause 8 is documented",
        }
    }
}

impl fmt::Display for Criterion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}
