use {
    super::catalog::{Catalog, Page},
    crate::publication::{
        collect::page::slug,
        reference::{ReferenceClassifier, ReferenceKind},
    },
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    std::collections::HashMap,
};

pub(super) struct Heading {
    pub title:  String,
    pub anchor: String,
    pub level:  usize,
}
pub(super) struct Content {
    pub html:     String,
    pub headings: Vec<Heading>,
}
pub(super) fn headings(body: &str) -> Vec<Heading> {
    let events: Vec<_> =
        Parser::new_ext(body, crate::publication::markdown::options())
            .collect();
    index(&events)
}
fn index(events: &[Event<'_>]) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut occurrences = HashMap::new();
    for (i, event) in events.iter().enumerate() {
        if let Event::Start(Tag::Heading { level, .. }) = event {
            let mut title = String::new();
            for event in &events[i + 1..] {
                match event {
                    Event::End(TagEnd::Heading(_)) => break,
                    Event::Text(text) | Event::Code(text) => {
                        title.push_str(text)
                    },
                    _ => {},
                }
            }
            let base = slug(&title);
            let occurrence = occurrences.entry(base.clone()).or_insert(0usize);
            let anchor = if *occurrence == 0 {
                base
            } else {
                format!("{base}-{occurrence}")
            };
            *occurrence += 1;
            headings.push(Heading { title, anchor, level: *level as usize });
        }
    }
    headings
}
pub(super) fn render(
    body: &str,
    page: &Page,
    catalog: &Catalog,
) -> Result<Content> {
    let body = catalog.abbreviations.clean(body)?;
    let body = crate::publication::footnotes::complete(&body, &body)?;
    let mut events = crate::publication::mermaid::events(
        crate::publication::markdown::semantic_events(
            Parser::new_ext(&body, crate::publication::markdown::options())
                .collect(),
        ),
    )?;
    let headings = index(&events);
    let mut heading_index = 0;
    let mut image_presentations = Vec::new(); // bounded: one capped Markdown page.
    let mut unlinked = Vec::new(); // bounded: links of one capped Markdown page.
    for (index, event) in events.iter_mut().enumerate() {
        match event {
            Event::Start(Tag::Heading { id, .. }) => {
                *id = Some(headings[heading_index].anchor.clone().into());
                heading_index += 1;
            },
            Event::Start(Tag::Link { dest_url, .. }) => {
                if crate::publication::badge::label(dest_url).is_none() &&
                    crate::publication::anchor::label(dest_url).is_none() &&
                    crate::publication::temporal::lexical(dest_url).is_none() &&
                    crate::publication::content_blocks::table_reference(
                        dest_url,
                    )
                    .is_none()
                {
                    match link(dest_url, page, catalog)? {
                        Some(href) => *dest_url = href.into(),
                        None => unlinked.push(index),
                    }
                }
            },
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                if crate::publication::icon::Icon::parse_uri(dest_url)?
                    .is_some()
                {
                    continue;
                }
                let hints =
                    crate::publication::image_hints::ImageHints::parse(title)?;
                let key = (page.id.as_str().into(), dest_url.to_string());
                let (_, remote, id) =
                    catalog.references.get(&key).ok_or_else(|| {
                        anyhow::anyhow!(
                            "image is not retained in the publication KG: \
                             {dest_url}"
                        )
                    })?;
                if id.is_none() &&
                    url::Url::parse(remote).is_ok_and(|url| {
                        crate::publication::reference::is_badge(&url)
                    })
                {
                    *dest_url = remote.clone().into();
                    continue;
                }
                let id = id
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("image asset is missing"))?;
                let asset = catalog.pages.get(id).ok_or_else(|| {
                    anyhow::anyhow!("image asset is not selected: {dest_url}")
                })?;
                let projection = if hints.interactive_svg {
                    ensure!(
                        catalog.presentations.contains(&key),
                        "interactive SVG marker lacks its collected \
                         presentation hint"
                    );
                    Some(
                        catalog
                            .projections
                            .get(id)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "interactive SVG projection is missing"
                                )
                            })?
                            .clone(),
                    )
                } else {
                    None
                };
                let path = if let Some(projection) = &projection {
                    super::interactive_svg::path(page, projection)
                } else {
                    format!(
                        "{}{}",
                        "../".repeat(page.route.matches('/').count()),
                        asset.route
                    )
                };
                if projection.is_some() || hints.color_scheme.is_some() {
                    image_presentations.push((
                        index,
                        projection,
                        path.clone(),
                        hints.title.clone(),
                        hints.color_scheme,
                    ));
                }
                *dest_url = path.into();
                // The shared HTML presentation captions a figure.
                *title = if hints.figure {
                    hints.encoded()?.into()
                } else {
                    hints.title.unwrap_or_default().into()
                };
            },
            Event::Html(_) |
            Event::InlineHtml(_) |
            Event::Start(Tag::HtmlBlock) |
            Event::End(TagEnd::HtmlBlock)
                if catalog.link_policy ==
                    crate::publication::reference::LinkPolicy::Lenient =>
            {
                // Zero configuration publishes what it can: raw HTML is
                // dropped with a diagnostic; its Markdown content remains.
                if let Event::Html(html) | Event::InlineHtml(html) = event {
                    tracing::warn!(
                        code = "raw-html-dropped",
                        "{}: raw HTML dropped: {}",
                        page.source,
                        html.trim()
                    );
                }
                *event = Event::Text("".into());
            },
            Event::Html(_) |
            Event::InlineHtml(_) |
            Event::Start(Tag::HtmlBlock) => {
                anyhow::bail!(
                    "{}: raw HTML must be represented by a supported \
                     documentation construct",
                    page.source
                )
            },
            Event::Start(Tag::Table(columns)) => {
                ensure!(
                    columns.len() <= 32,
                    "website table exceeds column budget"
                )
            },
            _ => {},
        }
    }
    unlink(&mut events, &unlinked);
    for (index, projection, path, title, scheme) in image_presentations {
        super::interactive_svg::replace(
            &mut events,
            index,
            projection.as_ref(),
            &path,
            title.as_deref(),
            scheme,
        )?;
    }
    let events = super::super::class_diagram::resolve(events, |iri| {
        link(iri, page, catalog)?.ok_or_else(|| {
            anyhow::anyhow!("{}: unresolved diagram link {iri}", page.source)
        })
    })?;
    let mut html = String::new();
    pulldown_cmark::html::push_html(
        &mut html,
        catalog
            .abbreviations
            .html(
                catalog
                    .conformance
                    .html(super::super::markdown_html::events(
                        events.into_iter(),
                    )?)
                    .into_iter(),
            )?
            .into_iter(),
    );
    html = html.replace(
        "href=\"#grapharch-diagram-note\"",
        &format!(
            "href=\"{}{}#{}\"",
            "../".repeat(page.route.matches('/').count()),
            super::super::diagram_notes::ROUTE,
            super::super::diagram_notes::ID
        ),
    );
    Ok(Content { html, headings })
}
/// Render broken links as their text: drop each listed link start and its
/// matching end. Indices are positions of `Event::Start(Tag::Link)`.
fn unlink(events: &mut Vec<Event<'_>>, starts: &[usize]) {
    if starts.is_empty() {
        return;
    }
    let mut remove = vec![false; events.len()]; // bounded: one page's events.
    for &start in starts {
        remove[start] = true;
        let mut depth = 0usize;
        for (offset, event) in events[start + 1..].iter().enumerate() {
            match event {
                Event::Start(Tag::Link { .. }) => depth += 1,
                Event::End(TagEnd::Link) if depth == 0 => {
                    remove[start + 1 + offset] = true;
                    break;
                },
                Event::End(TagEnd::Link) => depth -= 1,
                _ => {},
            }
        }
    }
    let mut index = 0;
    events.retain(|_| {
        index += 1;
        !remove[index - 1]
    });
}

/// The href of one reference, decided by the reference classifier; `None`
/// for a broken reference under the lenient link policy.
pub(super) fn link(
    value: &str,
    page: &Page,
    catalog: &Catalog,
) -> Result<Option<String>> {
    let lookup = super::reference_lookup::PageLookup { page, catalog };
    let classification = ReferenceClassifier::new(&lookup, catalog.link_policy)
        .classify(value, &page.source, &page.route, false)?;
    catalog
        .recorder
        .record(&page.source, value, &classification)?;
    if let ReferenceKind::Broken(reason) = &classification.kind {
        tracing::warn!(
            code = "broken-reference",
            "{}: {reason}",
            page.source
        );
    }
    Ok(classification.href)
}
