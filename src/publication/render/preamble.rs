//! Target-owned legal front matter, read only from the pinned graph.
use {
    super::{
        markup::{Markdown, html_escape, quote},
        model::{Plan, title},
    },
    crate::publication::native_html,
    anyhow::{Result, ensure},
    oxrdf::Term,
};

struct Preamble {
    title: String,
    html:  String,
}

impl Preamble {
    async fn read(plan: &Plan) -> Result<Option<Self>> {
        let Some(value) =
            plan.source.optional(&plan.target, "preamble").await?
        else {
            return Ok(None);
        };
        let Term::NamedNode(fragment) = value else {
            anyhow::bail!("preamble must identify a Fragment")
        };
        ensure!(
            plan.source.has_type(&fragment, "Fragment").await?,
            "preamble must identify a Fragment"
        );
        let title = title(&plan.source, &fragment).await?;
        let body = plan.source.text(&fragment, "body").await?;
        ensure!(
            !body.trim().is_empty() && body.len() <= 256 * 1024,
            "preamble must be nonempty and at most 256 KiB"
        );
        let html = match plan.source.text(&fragment, "format").await?.as_str() {
            "text/html" => {
                native_html::read(&body, None, &mut |_| Ok(None))?.html
            },
            "text/markdown" => {
                let markdown =
                    Markdown::parse(&body, plan, "legal-notices", &fragment, 1)
                        .await?;
                let mut html = String::new();
                pulldown_cmark::html::push_html(
                    &mut html,
                    super::markdown_html::events(markdown.events.into_iter()),
                );
                html
            },
            format => anyhow::bail!("unsupported preamble format {format}"),
        };
        Ok(Some(Self { title, html }))
    }
}

pub(super) async fn html(plan: &Plan) -> Result<String> {
    Ok(match Preamble::read(plan).await? {
        Some(preamble) => {
            format!(
                "<details class=\"publication-preamble\"><summary>{}</\
                 summary><div class=\"legal-content\">{}</div></details>",
                html_escape(&preamble.title),
                preamble.html
            )
        },
        None => String::new(),
    })
}

pub(super) async fn typst(
    plan: &Plan,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let Some(preamble) = Preamble::read(plan).await? else {
        return Ok(String::new())
    };
    let body = super::native_typst::convert(&preamble.html, diagnostics)?;
    Ok(format!(
        r##"
#let ga-legal-notices(body) = {{
  set text(size: 9pt, fill: rgb("#374151"))
  set par(justify: true, leading: 0.65em, spacing: 0.9em)
  show heading: it => block(above: 1.5em, below: 0.7em, sticky: true)[#text(fill: ga-accent, weight: "semibold")[#it.body]]
  block(width: 100%, inset: (bottom: 14pt), stroke: (bottom: 1.5pt + ga-accent))[
    #text(size: 9pt, tracking: 1pt, fill: ga-accent)[LEGAL NOTICES]
    #v(8pt)
    #text(size: 24pt, weight: "bold")[#text({title})]
  ]
  v(14pt)
  body
}}
#ga-legal-notices[
{body}
]
#pagebreak()
"##,
        title = quote(&preamble.title)
    ))
}
