//! Standards-style HTML. Content and publication identity come from the pinned
//! graph; this renderer owns document structure, numbering and presentation.
mod authority;
mod config;
pub(in crate::publication::render) mod metadata;
mod native;
mod outline;
pub(super) mod pages;
mod rows;

use {
    super::{
        content::{Entries, Entry, code_language},
        markup::{Markdown, heading_level, html_escape},
        model::Plan,
    },
    crate::publication::{
        diagnostics::{Diagnostic, FailureMode, Severity},
        native_html,
    },
    anyhow::{Result, ensure},
    outline::{Outline, Toc, headings, markdown_html, permalink},
    std::{collections::HashSet, path::Path},
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};

pub(super) const STYLE: &str = include_str!("style.css");

pub(super) async fn write(
    plan: &Plan,
    stage: &Path,
    mode: FailureMode,
) -> Result<Vec<Diagnostic>> {
    // Validate publication identity before emitting any visible generation.
    let header = metadata::header(plan).await?;
    let mut file =
        BufWriter::new(File::create(stage.join("index.html")).await?);
    file.write_all(
        format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta \
             charset=\"utf-8\"><meta name=\"viewport\" \
             content=\"width=device-width,initial-scale=1\"><title>{}</\
             title><link rel=\"stylesheet\" href=\"style.css\"><link \
             rel=\"icon\" href=\"data:,\"><meta name=\"grapharch-revision\" \
             content=\"{}\"></head><body class=\"specification\" \
             style=\"--authority-color:{}\"><a class=\"skip-link\" \
             href=\"#document\">Skip to document</a>\n",
            html_escape(&plan.title),
            html_escape(plan.revision.as_str()),
            header.color
        )
        .as_bytes(),
    )
    .await?;
    file.write_all(format!("<main id=\"document\">{}", header.html).as_bytes())
        .await?;
    let mut entries = Entries::new(plan).await?;
    let mut outline = Outline::default();
    let mut open = false;
    let mut navigation = false;
    let mut diagnostics = Vec::new();
    while let Some(entry) = entries.next(plan).await? {
        if entry.first {
            if open {
                file.write_all(b"</section>\n").await?;
            }
            let label =
                outline.component(&entry.kind, &entry.title, entry.depth)?;
            if !navigation &&
                !matches!(entry.kind.as_str(), "abstract" | "status")
            {
                diagnostics = toc(&mut file, plan, mode).await?;
                navigation = true;
            }
            let id = plan.anchor(&entry.placement, None);
            let (level, aria) = heading_level(entry.depth + 1);
            let search_kind = pages::kind(&entry, plan).await?;
            file.write_all(
                format!(
                    "<section id=\"{id}\" \
                     data-search-kind=\"{search_kind}\"><h{level}{aria}>{}{}</\
                     h{level}>\n",
                    html_escape(&label),
                    permalink(&id)
                )
                .as_bytes(),
            )
            .await?;
            open = true;
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        for alias in native::aliases(&entry, plan).await? {
            file.write_all(
                format!("<span id=\"{}\"></span>", html_escape(&alias))
                    .as_bytes(),
            )
            .await?;
        }
        let body = if entry.format == "text/html" {
            native_html::read(&entry.body, None, &mut |depth| {
                outline.heading_number(depth)
            })?
            .html
        } else if let Some(markdown) = markdown(&entry, plan, &anchor).await? {
            markdown_html(&markdown, &mut outline)?
        } else {
            format!(
                "<pre><code class=\"language-{}\">{}</code></pre>",
                code_language(&entry.format)?,
                html_escape(&entry.body)
            )
        };
        file.write_all(
            format!(
                "<div class=\"fragment\" id=\"{anchor}\" data-fragment=\"{}\" \
                 data-format=\"{}\">{body}</div>\n",
                html_escape(entry.fragment.as_str()),
                html_escape(&entry.format)
            )
            .as_bytes(),
        )
        .await?;
    }
    outline.finish()?;
    if open {
        file.write_all(b"</section>\n").await?;
    }
    if !navigation {
        diagnostics = toc(&mut file, plan, mode).await?;
    }
    if !plan.quality.is_empty() {
        let title = outline.component("appendix", "Quality issues", 1)?;
        super::quality::html_titled(&mut file, &plan.quality, &title).await?;
    }
    if !diagnostics.is_empty() {
        file.write_all(b"<section id=\"publication-diagnostics\"><h2>Publication diagnostics</h2><p>This draft contains unresolved documentation errors.</p><ol>").await?;
        for error in &diagnostics {
            file.write_all(
                format!(
                    "<li><strong>{}</strong>: {} <code>{}</code></li>",
                    html_escape(&error.code),
                    html_escape(&error.message),
                    html_escape(&error.reference)
                )
                .as_bytes(),
            )
            .await?;
        }
        file.write_all(b"</ol></section>").await?;
    }
    file.write_all(include_str!("search.html").as_bytes())
        .await?;
    file.write_all(
        format!("<footer>{}</footer>", super::credit::HTML).as_bytes(),
    )
    .await?;
    file.write_all(b"</main><div id=\"toc-resizer\" class=\"toc-resizer\" role=\"separator\" aria-label=\"Resize table of contents\" aria-controls=\"toc\" aria-orientation=\"vertical\" tabindex=\"0\" title=\"Drag or use arrow keys to resize; double-click to reset\" hidden></div><script>").await?;
    file.write_all(include_str!("sidebar.js").as_bytes())
        .await?;
    file.write_all(b"</script></body></html>\n").await?;
    file.flush().await?;
    diagnostics.extend(plan.quality.clone());
    super::content::write_typst(plan, stage, &diagnostics).await?;
    Ok(diagnostics)
}

async fn toc(
    file: &mut BufWriter<File>,
    plan: &Plan,
    mode: FailureMode,
) -> Result<Vec<Diagnostic>> {
    file.write_all(b"<nav id=\"toc\" class=\"toc\" role=\"doc-toc\" aria-labelledby=\"toc-title\"><h2 id=\"toc-title\">Table of Contents</h2>").await?;
    file.write_all(include_str!("navigation.html").as_bytes())
        .await?;
    let mut entries = Entries::new(plan).await?;
    let mut outline = Outline::default();
    let mut toc = Toc::new();
    let front_matter = config::flag(plan, "tocFrontMatter").await?;
    let toc_depth = plan.toc_depth().await?.unwrap_or(usize::MAX);
    let mut ids: HashSet<String> = ["document", "toc", "toc-title"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if !plan.quality.is_empty() {
        ids.insert("quality-issues".into());
    }
    let mut links = Vec::new();
    while let Some(entry) = entries.next(plan).await? {
        if entry.first {
            let label =
                outline.component(&entry.kind, &entry.title, entry.depth)?;
            if (front_matter ||
                !matches!(entry.kind.as_str(), "abstract" | "status")) &&
                entry.depth <= toc_depth
            {
                file.write_all(
                    toc.entry(
                        entry.depth,
                        &plan.anchor(&entry.placement, None),
                        &label,
                    )?
                    .as_bytes(),
                )
                .await?;
            }
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        for id in native::aliases(&entry, plan).await?.into_iter().chain(
            [anchor.clone(), plan.anchor(&entry.placement, None)]
                .into_iter()
                .take(if entry.first { 2 } else { 1 }),
        ) {
            ensure!(
                ids.insert(id.clone()),
                "duplicate publication anchor {id}"
            );
        }
        if entry.format == "text/html" {
            let fragment =
                native_html::read(&entry.body, None, &mut |_| Ok(None))?;
            for id in fragment.ids {
                ensure!(
                    ids.insert(id.clone()),
                    "duplicate publication anchor {id}"
                );
            }
            links.extend(
                fragment
                    .links
                    .into_iter()
                    .map(|link| (link, entry.fragment.as_str().to_owned())),
            );
            for heading in fragment.headings {
                if !heading.toc {
                    continue;
                }
                let number = if heading.numbered {
                    outline.heading_number(heading.depth)?
                } else {
                    None
                };
                let label = if let Some(number) = number {
                    format!("{number} {}", heading.text)
                } else {
                    heading.text
                };
                if (front_matter ||
                    !matches!(entry.kind.as_str(), "abstract" | "status")) &&
                    heading.depth + entry.depth <= toc_depth
                {
                    file.write_all(
                        toc.entry(
                            heading.depth + entry.depth,
                            &heading.id,
                            &label,
                        )?
                        .as_bytes(),
                    )
                    .await?;
                }
            }
        } else if let Some(markdown) = markdown(&entry, plan, &anchor).await? {
            for heading in headings(&markdown)? {
                let number = outline.heading_number(heading.depth)?;
                let label = if let Some(number) = number {
                    format!("{number} {}", heading.text)
                } else {
                    heading.text
                };
                if (front_matter ||
                    !matches!(entry.kind.as_str(), "abstract" | "status")) &&
                    heading.depth + entry.depth <= toc_depth
                {
                    file.write_all(
                        toc.entry(
                            heading.depth + entry.depth,
                            &heading.id,
                            &label,
                        )?
                        .as_bytes(),
                    )
                    .await?;
                }
            }
        }
    }
    outline.finish()?;
    ensure!(
        ids.len() <= crate::publication::limits::SPECIFICATION_ANCHORS &&
            links.len() <= crate::publication::limits::SPECIFICATION_LINKS,
        "specification exceeds native anchor/link budget"
    );
    let mut diagnostics = Vec::new();
    let mut seen_errors = HashSet::new();
    for (link, source) in links {
        if let Some(id) = link.strip_prefix('#') &&
            !ids.contains(id)
        {
            ensure!(
                mode == FailureMode::KeepGoing,
                "unresolved native reference {link}"
            );
            if seen_errors.insert((link.clone(), source.clone())) {
                ensure!(
                    diagnostics.len() < 1024,
                    "diagnostic budget exceeded"
                );
                diagnostics.push(Diagnostic {
                    severity:        Severity::Error,
                    code:            "unresolved-native-reference".into(),
                    message:         format!(
                        "No selected content defines the anchor {link}"
                    ),
                    reference:       link,
                    source_fragment: Some(source),
                });
            }
        }
    }
    if !plan.quality.is_empty() {
        file.write_all(
            toc.entry(
                1,
                "quality-issues",
                &outline.component("appendix", "Quality issues", 1)?,
            )?
            .as_bytes(),
        )
        .await?;
    }
    file.write_all(format!("{}</nav>\n", toc.finish()).as_bytes())
        .await?;
    Ok(diagnostics)
}

async fn markdown<'a>(
    entry: &'a Entry,
    plan: &Plan,
    anchor: &str,
) -> Result<Option<Markdown<'a>>> {
    if entry.format == "text/markdown" {
        Ok(Some(
            Markdown::parse(
                &entry.body,
                plan,
                anchor,
                &entry.fragment,
                entry.depth,
            )
            .await?,
        ))
    } else {
        Ok(None)
    }
}

pub(super) async fn pdf_metadata(
    plan: &Plan,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<String> {
    let header = metadata::header(plan).await?;
    let document = scraper::Html::parse_fragment(&header.html);
    let mut html = String::new();
    for part in document.select(
        &scraper::Selector::parse(
            ".publication-status,.publication-metadata,.copyright",
        )
        .unwrap(),
    ) {
        html.push_str(&part.html());
    }
    super::native_typst::convert(&html, diagnostics)
}
