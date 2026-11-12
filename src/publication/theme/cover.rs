//! A declarative cover: ordered blocks, each placing one element of the
//! target's publication identity with validated presentation values.
use {
    super::values::{Kind, boolean, integer},
    crate::publication::{NAMESPACE, RDF_TYPE, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

/// What a cover block shows. Values come from the target, never the theme,
/// except for [`CoverElement::Text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoverElement {
    /// Vertical space.
    Space,
    /// The publishing authority's name.
    Publisher,
    /// The authority's `ga:logo`.
    Logo,
    /// `ga:publicationDate` in the block's date format.
    Date,
    /// The document title.
    Title,
    /// `ga:subtitle`, when the target declares one.
    Subtitle,
    /// `ga:releaseVersion` inserted into the block's text pattern.
    Version,
    /// A horizontal rule.
    Rule,
    /// A short filled bar.
    Bar,
    /// The target's ordered `ga:metadataRow` label and value rows.
    Rows,
    /// The renderer's publication summary: status, release and description.
    Summary,
    /// Fixed text supplied by the theme.
    Text,
}

impl CoverElement {
    fn from_local(local: &str) -> Option<Self> {
        Some(match local {
            "CoverSpace" => Self::Space,
            "CoverPublisher" => Self::Publisher,
            "CoverLogo" => Self::Logo,
            "CoverDate" => Self::Date,
            "CoverTitle" => Self::Title,
            "CoverSubtitle" => Self::Subtitle,
            "CoverVersion" => Self::Version,
            "CoverRule" => Self::Rule,
            "CoverBar" => Self::Bar,
            "CoverRows" => Self::Rows,
            "CoverSummary" => Self::Summary,
            "CoverText" => Self::Text,
            _ => return None,
        })
    }

    fn typeset(self) -> bool {
        matches!(
            self,
            Self::Publisher |
                Self::Date |
                Self::Title |
                Self::Subtitle |
                Self::Version |
                Self::Rows |
                Self::Summary |
                Self::Text
        )
    }
}

/// How a cover date is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DateFormat {
    /// `March 2026`.
    MonthYear,
    /// `31 March 2026`.
    DayMonthYear,
    /// `2026-03-31`.
    Iso,
}

/// One validated cover block. Presentation fields hold Typst expressions.
pub(crate) struct CoverBlock {
    pub order:       u32,
    pub element:     CoverElement,
    pub size:        Option<String>,
    pub weight:      Option<String>,
    pub italic:      bool,
    pub align:       Option<String>,
    pub color:       Option<String>,
    pub font:        Option<String>,
    pub tracking:    Option<String>,
    pub width:       Option<String>,
    pub height:      Option<String>,
    pub stroke:      Option<String>,
    pub space:       Option<String>,
    pub panel:       bool,
    pub text:        Option<String>,
    pub pattern:     Option<String>,
    pub date_format: Option<DateFormat>,
}

impl CoverBlock {
    pub(super) async fn read(
        view: &RevisionView,
        block: &NamedNode,
    ) -> Result<Self> {
        // bounded: a cover block has at most 32 statements.
        let mut values = BTreeMap::new();
        let mut rows = view.statements(block).await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(predicate) = &row["predicate"] else {
                bail!("cover block predicate must be an IRI")
            };
            if predicate.as_str() == RDF_TYPE {
                continue;
            }
            let Some(local) = predicate.as_str().strip_prefix(NAMESPACE) else {
                continue;
            };
            ensure!(
                values.len() < 32,
                "cover block {block} is too large"
            );
            ensure!(
                values
                    .insert(local.to_owned(), row["value"].clone())
                    .is_none(),
                "cover block {block} repeats ga:{local}"
            );
        }
        let element = match values.remove("coverElement") {
            Some(Term::NamedNode(element)) => {
                element
                    .as_str()
                    .strip_prefix(NAMESPACE)
                    .and_then(CoverElement::from_local)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "cover block {block} has unsupported element \
                             {element}"
                        )
                    })?
            },
            _ => bail!("cover block {block} requires a ga:coverElement IRI"),
        };
        let order = integer(
            "order",
            &values.remove("order").ok_or_else(|| {
                anyhow::anyhow!("cover block {block} requires ga:order")
            })?,
        )?;
        let mut take = |name: &str, kind: Kind| -> Result<Option<String>> {
            values
                .remove(name)
                .map(|value| kind.typst(name, &value))
                .transpose()
        };
        let size = take("fontSize", Kind::Length)?;
        let weight = take("fontWeight", Kind::Weight)?;
        let align = take("alignment", Kind::Align)?;
        let color = take("color", Kind::Color)?;
        let font = take("fontFamily", Kind::Font)?;
        let tracking = take("tracking", Kind::Length)?;
        let width = take("width", Kind::Relative)?;
        let height = take("height", Kind::Length)?;
        let stroke = take("stroke", Kind::Length)?;
        let space = take("space", Kind::Spacing)?;
        let mut raw = |name: &str, kind: Kind| -> Result<Option<String>> {
            let Some(value) = values.remove(name) else {
                return Ok(None)
            };
            kind.typst(name, &value)?;
            let Term::Literal(value) = value else {
                unreachable!("validated literal")
            };
            Ok(Some(value.value().to_owned()))
        };
        let text = raw("coverText", Kind::Text)?;
        let pattern = raw("textPattern", Kind::Pattern("{version}"))?;
        let italic = values
            .remove("italic")
            .map(|value| boolean("italic", &value))
            .transpose()?
            .unwrap_or(false);
        let panel = values
            .remove("panel")
            .map(|value| boolean("panel", &value))
            .transpose()?
            .unwrap_or(false);
        let date_format = match values.remove("dateFormat") {
            None => None,
            Some(Term::NamedNode(format)) => {
                Some(match format.as_str().strip_prefix(NAMESPACE) {
                    Some("MonthYearDate") => DateFormat::MonthYear,
                    Some("DayMonthYearDate") => DateFormat::DayMonthYear,
                    Some("IsoDate") => DateFormat::Iso,
                    _ => {
                        bail!("cover block {block} has unsupported date format")
                    },
                })
            },
            Some(_) => {
                bail!("cover block {block} ga:dateFormat must be an IRI")
            },
        };
        ensure!(
            values.is_empty(),
            "cover block {block} has unsupported properties: {}",
            values.keys().cloned().collect::<Vec<_>>().join(", ")
        );
        let block_name = block.as_str();
        let need = |present: bool, field: &str| -> Result<()> {
            ensure!(
                present,
                "cover block {block_name} requires ga:{field}"
            );
            Ok(())
        };
        match element {
            CoverElement::Space => need(space.is_some(), "space")?,
            CoverElement::Logo => need(height.is_some(), "height")?,
            CoverElement::Rule => {
                need(stroke.is_some(), "stroke")?;
                need(color.is_some(), "color")?;
            },
            CoverElement::Bar => {
                need(width.is_some(), "width")?;
                need(height.is_some(), "height")?;
                need(color.is_some(), "color")?;
            },
            CoverElement::Date => need(date_format.is_some(), "dateFormat")?,
            CoverElement::Version => need(pattern.is_some(), "textPattern")?,
            CoverElement::Text => need(text.is_some(), "coverText")?,
            _ => {},
        }
        if element.typeset() {
            need(size.is_some(), "fontSize")?;
        }
        Ok(Self {
            order,
            element,
            size,
            weight,
            italic,
            align,
            color,
            font,
            tracking,
            width,
            height,
            stroke,
            space,
            panel,
            text,
            pattern,
            date_format,
        })
    }
}
