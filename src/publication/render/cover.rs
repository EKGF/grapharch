//! Typesets the theme's declarative cover from the target's identity.
use {
    super::{
        markup::quote,
        model::Plan,
        native_typst,
        publication_date::PublicationDate,
        specification::{
            authority::Logo,
            rows::{self, RowValue},
        },
    },
    crate::publication::theme::{
        CoverBlock,
        CoverElement,
        DateFormat,
        PdfTheme,
    },
    anyhow::{Result, bail},
    oxrdf::Term,
    std::path::Path,
};

/// The file name of a written cover logo, beside `main.typ`.
const LOGO: &str = "cover-logo";

/// Target-owned values the cover can show.
pub(super) struct CoverContent {
    pub publisher: String,
    pub title:     String,
    /// Typst markup for `ga:subtitle`, when declared.
    pub subtitle:  Option<String>,
    /// Typst markup for the renderer's publication summary.
    pub summary:   String,
}

/// Write the authority logo beside the Typst source when the theme's cover
/// shows it. Returns the published file name.
pub(super) async fn write_logo(
    plan: &Plan,
    theme: &PdfTheme,
    stage: &Path,
) -> Result<Option<String>> {
    if !theme
        .cover()
        .iter()
        .any(|block| block.element == CoverElement::Logo)
    {
        return Ok(None);
    }
    let logo = logo(plan).await?;
    let name = format!("{LOGO}.{}", logo.extension);
    tokio::fs::write(stage.join(&name), &logo.bytes).await?;
    Ok(Some(name))
}

async fn logo(plan: &Plan) -> Result<Logo> {
    let Some(Term::NamedNode(publisher)) =
        plan.source.optional(&plan.target, "publisher").await?
    else {
        bail!("the theme's cover logo requires a ga:publisher authority")
    };
    Logo::read(plan, &publisher).await?.ok_or_else(|| {
        anyhow::anyhow!(
            "the theme's cover logo requires the publisher's ga:logo"
        )
    })
}

pub(super) async fn typst(
    plan: &Plan,
    theme: &PdfTheme,
    content: &CoverContent,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let mut output = String::from(
        "#[\n#set par(justify: false, leading: 0.45em)\n#set text(font: \
         ga-theme.cover_font)\n",
    );
    for block in theme.cover() {
        output.push_str(&element(plan, block, content, diagnostics).await?);
    }
    output.push_str("]\n");
    Ok(output)
}

fn styled(block: &CoverBlock, body: &str) -> String {
    let mut arguments = Vec::new();
    for (name, value) in [
        ("size", &block.size),
        ("weight", &block.weight),
        ("fill", &block.color),
        ("font", &block.font),
        ("tracking", &block.tracking),
    ] {
        if let Some(value) = value {
            arguments.push(format!("{name}: {value}"));
        }
    }
    if block.italic {
        arguments.push("style: \"italic\"".into());
    }
    let text = format!("#text({})[{body}]", arguments.join(", "));
    aligned(block, &format!("#block(width: 100%)[{text}]"))
}

fn aligned(block: &CoverBlock, body: &str) -> String {
    match &block.align {
        Some(align) => format!("#align({align})[{body}]\n"),
        None => format!("{body}\n"),
    }
}

fn plain(value: &str) -> String { format!("#text({})", quote(value)) }

async fn element(
    plan: &Plan,
    block: &CoverBlock,
    content: &CoverContent,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    Ok(match block.element {
        CoverElement::Space => {
            format!(
                "#v({})\n",
                block.space.as_deref().unwrap_or("0pt")
            )
        },
        CoverElement::Publisher => styled(block, &plain(&content.publisher)),
        CoverElement::Title => styled(block, &plain(&content.title)),
        CoverElement::Subtitle => {
            match &content.subtitle {
                Some(subtitle) => styled(block, subtitle),
                None => String::new(),
            }
        },
        CoverElement::Text => {
            styled(
                block,
                &plain(block.text.as_deref().unwrap_or_default()),
            )
        },
        CoverElement::Date => {
            let date = PublicationDate::read(plan).await?.ok_or_else(|| {
                anyhow::anyhow!(
                    "the theme's cover date requires ga:publicationDate"
                )
            })?;
            let text = match block.date_format.expect("validated date format") {
                DateFormat::MonthYear => date.month_year(),
                DateFormat::DayMonthYear => date.long(),
                DateFormat::Iso => date.iso(),
            };
            styled(block, &plain(&text))
        },
        CoverElement::Version => {
            let version = match plan
                .source
                .optional(&plan.target, "releaseVersion")
                .await?
            {
                Some(Term::Literal(version)) => version.value().to_owned(),
                _ => {
                    bail!(
                        "the theme's cover version line requires \
                         ga:releaseVersion"
                    )
                },
            };
            let pattern = block.pattern.as_deref().unwrap_or_default();
            styled(
                block,
                &plain(&pattern.replace("{version}", &version)),
            )
        },
        CoverElement::Logo => {
            let logo = logo(plan).await?;
            let name = crate::publication::governance::name(
                &plan.source,
                &plan.source.node(&plan.target, "publisher").await?,
            )
            .await?;
            aligned(
                block,
                &format!(
                    "#image({}, height: {}, alt: {})",
                    quote(&format!("{LOGO}.{}", logo.extension)),
                    block.height.as_deref().unwrap_or("1in"),
                    quote(&name)
                ),
            )
        },
        CoverElement::Rule => {
            format!(
                "#line(length: {}, stroke: {} + {})\n",
                block.width.as_deref().unwrap_or("100%"),
                block.stroke.as_deref().unwrap_or("0.5pt"),
                block.color.as_deref().unwrap_or("ga-ink")
            )
        },
        CoverElement::Bar => {
            format!(
                "#rect(width: {}, height: {}, fill: {}, stroke: none)\n",
                block.width.as_deref().unwrap_or("16mm"),
                block.height.as_deref().unwrap_or("1.5mm"),
                block.color.as_deref().unwrap_or("ga-accent")
            )
        },
        CoverElement::Rows => rows(plan, block, diagnostics).await?,
        CoverElement::Summary => {
            if content.summary.trim().is_empty() {
                String::new()
            } else if block.panel {
                format!(
                    "#block(width: 100%, inset: (x: 6mm, y: 5mm), fill: \
                     ga-soft, radius: (top-right: 4mm, bottom-left: \
                     4mm))[\n#set text(size: {})\n#set par(leading: 0.6em, \
                     spacing: 0.7em)\n{}]\n",
                    block.size.as_deref().unwrap_or("9pt"),
                    content.summary
                )
            } else {
                format!(
                    "#block(width: 100%)[\n#set text(size: {})\n{}]\n",
                    block.size.as_deref().unwrap_or("9pt"),
                    content.summary
                )
            }
        },
    })
}

async fn rows(
    plan: &Plan,
    block: &CoverBlock,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let rows = rows::read(plan).await?;
    anyhow::ensure!(
        !rows.is_empty(),
        "the theme's cover rows require ordered ga:metadataRow entries"
    );
    let mut cells = Vec::new();
    for row in rows {
        let value = match row.value {
            RowValue::Text(value) => plain(&value),
            RowValue::Link { url, label } => {
                format!("#link({})[{}]", quote(&url), plain(&label))
            },
            RowValue::People { names, .. } => plain(&names.join(", ")),
            RowValue::Html(html) => native_typst::convert(&html, diagnostics)?,
        };
        cells.push(format!("[{}], [{value}]", plain(&row.label)));
    }
    let mut arguments = Vec::new();
    for (name, value) in [
        ("size", &block.size),
        ("weight", &block.weight),
        ("fill", &block.color),
        ("font", &block.font),
    ] {
        if let Some(value) = value {
            arguments.push(format!("{name}: {value}"));
        }
    }
    Ok(format!(
        "#block(width: 100%)[#set text({})\n#show link: set text(fill: \
         ga-ink)\n#grid(columns: (auto, 1fr), column-gutter: 1.3em, \
         row-gutter: 0.9em, {})]\n",
        arguments.join(", "),
        cells.join(", ")
    ))
}
