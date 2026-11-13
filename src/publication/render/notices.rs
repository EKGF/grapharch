//! Target-owned front-matter notices, in declared order, read only from the
//! pinned graph. A notice is either a fragment (legal terms, procedures) or
//! an ordered list of copyright holders. Their wording is source content; the
//! theme only styles it.
use {
    super::{
        markup::{Markdown, html_escape, quote},
        model::{Plan, title},
    },
    crate::publication::{native_html, validation::unsigned},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

const MAX_NOTICES: usize = 32;
const MAX_HOLDERS: usize = 64;

struct Holder {
    order: u32,
    name:  String,
    years: String,
}

impl Holder {
    async fn read(plan: &Plan, holder: &NamedNode) -> Result<Self> {
        let order =
            unsigned(
                &plan.source.optional(holder, "order").await?.ok_or_else(
                    || {
                        anyhow::anyhow!(
                            "copyright holder {holder} requires ga:order"
                        )
                    },
                )?,
                "copyright holder order",
            )?;
        let name = plan.source.text(holder, "holderName").await?;
        let years = plan.source.text(holder, "copyrightYears").await?;
        ensure!(
            !name.trim().is_empty() &&
                name.len() <= 256 &&
                !years.trim().is_empty() &&
                years.len() <= 64,
            "copyright holder {holder} requires a name and years"
        );
        Ok(Self { order, name, years })
    }

    fn line(&self) -> String {
        format!("Copyright © {}, {}", self.years, self.name)
    }
}

enum Content {
    Fragment { title: String, html: String },
    Copyright(Vec<Holder>),
}

struct Notice {
    order:   u32,
    content: Content,
}

impl Notice {
    async fn read(plan: &Plan, notice: &NamedNode) -> Result<Self> {
        let order =
            unsigned(
                &plan.source.optional(notice, "order").await?.ok_or_else(
                    || anyhow::anyhow!("notice {notice} requires ga:order"),
                )?,
                "notice order",
            )?;
        let fragment = plan.source.optional(notice, "fragment").await?;
        let mut holders = Vec::new();
        let mut rows = plan.source.objects(notice, "copyrightHolder").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                holders.len() < MAX_HOLDERS,
                "notice {notice} exceeds {MAX_HOLDERS} copyright holders"
            );
            let Term::NamedNode(holder) = &row["value"] else {
                bail!("copyrightHolder must be an IRI")
            };
            holders.push(Holder::read(plan, holder).await?);
        }
        let content = match (fragment, holders.is_empty()) {
            (Some(Term::NamedNode(fragment)), true) => {
                Self::fragment(plan, &fragment).await?
            },
            (None, false) => {
                holders.sort_by_key(|holder| holder.order);
                ensure!(
                    holders
                        .windows(2)
                        .all(|pair| pair[0].order != pair[1].order),
                    "notice {notice} has ambiguous copyright holder orders"
                );
                Content::Copyright(holders)
            },
            _ => {
                bail!(
                    "notice {notice} requires exactly one ga:fragment or \
                     ordered ga:copyrightHolder entries"
                )
            },
        };
        Ok(Self { order, content })
    }

    async fn fragment(plan: &Plan, fragment: &NamedNode) -> Result<Content> {
        ensure!(
            plan.source.has_type(fragment, "Fragment").await?,
            "notice must identify a Fragment"
        );
        let title = title(&plan.source, fragment).await?;
        let body = plan.source.text(fragment, "body").await?;
        ensure!(
            !body.trim().is_empty() && body.len() <= 256 * 1024,
            "notice must be nonempty and at most 256 KiB"
        );
        let html = match plan.source.text(fragment, "format").await?.as_str() {
            "text/html" => {
                native_html::read(
                    &body,
                    native_html::Links::Publication(&plan.targets),
                    &mut |_| Ok(None),
                )?
                .html
            },
            "text/markdown" => {
                let markdown =
                    Markdown::parse(&body, plan, "notices", fragment, 1)
                        .await?;
                let mut html = String::new();
                pulldown_cmark::html::push_html(
                    &mut html,
                    plan.abbreviations
                        .html(super::markdown_html::events(
                            markdown.events.into_iter(),
                        )?)?
                        .into_iter(),
                );
                html
            },
            format => bail!("unsupported notice format {format}"),
        };
        Ok(Content::Fragment { title, html })
    }
}

async fn read(plan: &Plan) -> Result<Vec<Notice>> {
    let mut notices = Vec::new();
    let mut rows = plan.source.objects(&plan.target, "notice").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            notices.len() < MAX_NOTICES,
            "target exceeds {MAX_NOTICES} notices"
        );
        let Term::NamedNode(notice) = &row["value"] else {
            bail!("ga:notice must identify a notice IRI")
        };
        notices.push(Notice::read(plan, notice).await?);
    }
    notices.sort_by_key(|notice| notice.order);
    ensure!(
        notices
            .windows(2)
            .all(|pair| pair[0].order != pair[1].order),
        "target notices have ambiguous orders"
    );
    Ok(notices)
}

pub(super) async fn html(plan: &Plan) -> Result<String> {
    let mut html = String::new();
    for notice in read(plan).await? {
        match notice.content {
            Content::Fragment { title, html: body } => {
                html.push_str(&format!(
                    "<details class=\"publication-preamble\"><summary>{}</\
                     summary><div \
                     class=\"legal-content\">{body}</div></details>",
                    html_escape(&title)
                ));
            },
            Content::Copyright(holders) => {
                html.push_str("<ul class=\"copyright-holders\">");
                for holder in holders {
                    html.push_str(&format!(
                        "<li>{}</li>",
                        html_escape(&holder.line())
                    ));
                }
                html.push_str("</ul>");
            },
        }
    }
    Ok(html)
}

pub(super) async fn typst(
    plan: &Plan,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let notices = read(plan).await?;
    if notices.is_empty() {
        return Ok(String::new());
    }
    let mut body = String::new();
    for notice in notices {
        match notice.content {
            Content::Fragment { title, html } => {
                body.push_str(&format!(
                    "#ga-notice-title[#text({})]\n",
                    quote(&title)
                ));
                body.push_str(&notice_body(&html, diagnostics)?);
            },
            Content::Copyright(holders) => {
                body.push_str(
                    "#block(below: 2.5em)[\n#set par(spacing: 0.35em)\n",
                );
                for holder in holders {
                    body.push_str(&format!(
                        "#par[#text({})]\n",
                        quote(&holder.line())
                    ));
                }
                body.push_str("]\n");
            },
        }
    }
    Ok(format!("\n#ga-notices[\n{body}]\n#pagebreak()\n"))
}

/// Notice headings take the theme's notice heading style; the text between
/// them is converted as ordinary native content.
fn notice_body(
    html: &str,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let heading = regex::Regex::new(r"(?s)<h[1-6][^>]*>(.*?)</h[1-6]>")?;
    let mut output = String::new();
    let mut rest = 0;
    for found in heading.captures_iter(html) {
        let whole = found.get(0).expect("match");
        let chunk = &html[rest..whole.start()];
        if !chunk.trim().is_empty() {
            output.push_str(&super::native_typst::convert(chunk, diagnostics)?);
            output.push('\n');
        }
        let text = scraper::Html::parse_fragment(&found[1])
            .root_element()
            .text()
            .collect::<String>();
        output.push_str(&format!(
            "#ga-notice-heading(ga-theme.notice_heading_size)[#text({})]\n",
            quote(text.trim())
        ));
        rest = whole.end();
    }
    let chunk = &html[rest..];
    if !chunk.trim().is_empty() {
        output.push_str(&super::native_typst::convert(chunk, diagnostics)?);
        output.push('\n');
    }
    Ok(output)
}
