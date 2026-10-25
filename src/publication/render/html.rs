use {
    super::{
        content::{Entry, code_language, letter},
        markup::{Markdown, html_escape},
        model::{Plan, node, title},
    },
    anyhow::{Result, ensure},
    oxrdf::Term,
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};

pub(super) const STYLE: &str =
    "html{color:#172033;background:#fff;font:16px/1.7 \
     system-ui,sans-serif}body{max-width:1200px;margin:auto;padding:36px;\
     display:grid;grid-template-columns:230px \
     minmax(0,1fr);gap:50px}aside{position:sticky;top:24px;align-self:start;\
     font-size:14px}nav a{display:block;margin:8px \
     0}a{color:#1755a4;text-underline-offset:3px}h1,h2,h3{line-height:1.25;\
     color:#10243a}h1{font-size:2.7rem;margin:0 0 \
     12px}h2{font-size:1.7rem;margin-top:48px}header{padding-bottom:24px;\
     border-bottom:2px solid \
     #1755a4}.subtitle,footer{color:#526174}pre{overflow:auto;padding:18px;\
     background:#f3f5f8;border:1px solid \
     #dfe4eb;border-radius:4px}code{font:0.86em \
     ui-monospace,monospace}table{border-collapse:collapse;table-layout:fixed;\
     width:100%;margin:24px 0}th,td{border:1px solid #d4dbe5;padding:10px \
     14px;text-align:left;vertical-align:top}th{background:#\
     f3f5f8}blockquote{border-left:3px solid \
     #8ba7ca;padding-left:20px;margin-left:0;color:#415269}footer{margin-top:\
     60px;border-top:1px solid #dfe4eb;padding:20px 0;font-size:13px}.reader \
     main{max-width:760px;overflow-wrap:anywhere}.fragment{scroll-margin-top:\
     24px}@media(max-width:760px){body{display:block;padding:\
     22px}aside{position:static;margin-bottom:32px}h1{font-size:2rem}}@media \
     print{body{display:block;max-width:none;padding:0}aside{display:\
     none}pre{white-space:pre-wrap}a{color:inherit}}";

pub(super) async fn begin(
    file: &mut BufWriter<File>,
    plan: &Plan,
) -> Result<()> {
    let style = match plan.source.optional(&plan.profile, "style").await? {
        None => "specification".to_owned(),
        Some(Term::Literal(style)) => style.value().to_owned(),
        _ => anyhow::bail!("profile style must be a literal"),
    };
    ensure!(
        matches!(style.as_str(), "specification" | "reader"),
        "unsupported HTML profile style {style}"
    );
    file.write_all(
        format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta \
             charset=\"utf-8\"><meta name=\"viewport\" \
             content=\"width=device-width,initial-scale=1\"><title>{}</\
             title><link rel=\"stylesheet\" href=\"style.css\"><link \
             rel=\"icon\" href=\"data:,\"><meta name=\"grapharch-project\" \
             content=\"{}\"><meta name=\"grapharch-target\" \
             content=\"{}\"><meta name=\"grapharch-revision\" \
             content=\"{}\"><meta name=\"grapharch-scope\" \
             content=\"{}\"><meta name=\"grapharch-profile\" \
             content=\"{}\"></head><body class=\"{style}\"><aside><nav \
             aria-label=\"Contents\"><strong>Contents</strong>\n",
            html_escape(&plan.title),
            html_escape(plan.project.as_str()),
            html_escape(plan.target.as_str()),
            html_escape(plan.revision.as_str()),
            html_escape(plan.scope.as_str()),
            html_escape(plan.profile.as_str())
        )
        .as_bytes(),
    )
    .await?;
    let mut rows = plan.entries().await?;
    let mut previous = None;
    let mut appendix = 0u32;
    let mut count = 0u64;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 10_000,
            "target exceeds rendering entry budget"
        );
        let placement = node(&row["placement"])?;
        if previous.as_ref() == Some(placement) {
            continue;
        }
        let mut label = title(&plan.source, node(&row["component"])?).await?;
        if plan.section_kind(placement).await? == "appendix" {
            appendix += 1;
            label = format!("Appendix {}. {label}", letter(appendix));
        }
        file.write_all(
            format!(
                "<a href=\"#{}\">{}</a>\n",
                plan.anchor(placement, None),
                html_escape(&label)
            )
            .as_bytes(),
        )
        .await?;
        previous = Some(placement.clone());
    }
    file.write_all(
        format!(
            "</nav></aside><main><header><p \
             class=\"subtitle\">{}</p><h1>{}</h1></header>\n",
            html_escape(&title(&plan.source, &plan.project).await?),
            html_escape(&plan.title)
        )
        .as_bytes(),
    )
    .await?;
    Ok(())
}

pub(super) async fn entry(
    file: &mut BufWriter<File>,
    plan: &Plan,
    entry: &Entry,
    anchor: &str,
    markdown: Option<&Markdown<'_>>,
    appendix: u32,
) -> Result<()> {
    if entry.first {
        // Every first entry after the first opens a new component section.
        file.write_all(
            format!(
                "<!--component--><section id=\"{}\"><h2>{}</h2>\n",
                plan.anchor(&entry.placement, None),
                html_escape(
                    &if entry.kind == "appendix" {
                        format!(
                            "Appendix {}. {}",
                            letter(appendix),
                            entry.title
                        )
                    } else {
                        entry.title.clone()
                    }
                )
            )
            .as_bytes(),
        )
        .await?;
    }
    let body = if let Some(markdown) = markdown {
        markdown.html()
    } else {
        format!(
            "<pre><code class=\"language-{}\">{}</code></pre>",
            code_language(&entry.format)?,
            html_escape(&entry.body)
        )
    };
    file.write_all(
        format!(
            "<div class=\"fragment\" id=\"{anchor}\" \
             data-fragment=\"{}\">{body}</div>\n",
            html_escape(entry.fragment.as_str())
        )
        .as_bytes(),
    )
    .await?;
    Ok(())
}

pub(super) async fn end(
    file: &mut BufWriter<File>,
    has_entries: bool,
) -> Result<()> {
    if has_entries {
        close(file).await?;
    }
    file.write_all(b"<footer>Grapharch</footer></main></body></html>\n")
        .await?;
    Ok(())
}

pub(super) async fn close(file: &mut BufWriter<File>) -> Result<()> {
    file.write_all(b"</section>\n").await?;
    Ok(())
}
