use {
    super::catalog::{Catalog, Page},
    crate::publication::collect::page::slug,
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
                    *dest_url = link(dest_url, page, catalog)?.into();
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
                let (_, _, id) =
                    catalog.references.get(&key).ok_or_else(|| {
                        anyhow::anyhow!(
                            "image is not retained in the publication KG: \
                             {dest_url}"
                        )
                    })?;
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
        link(iri, page, catalog)
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
pub(super) fn link(
    value: &str,
    page: &Page,
    catalog: &Catalog,
) -> Result<String> {
    if let Some(href) = catalog
        .targets
        .resolve(value, &format!("{}index.html", page.route))?
    {
        return Ok(href);
    }
    // A page IRI links to the page itself.
    if let Some(target) =
        catalog.pages.get(value).filter(|target| !target.asset)
    {
        return Ok(if target.id == page.id {
            "#".into()
        } else {
            format!(
                "{}{}",
                "../".repeat(page.route.matches('/').count()),
                target.route
            )
        });
    }
    if let Some((id, anchor)) = catalog.subjects.get(value) {
        let target = &catalog.pages[id];
        return Ok(if target.id == page.id {
            format!("#{anchor}")
        } else {
            format!(
                "{}{}#{anchor}",
                "../".repeat(page.route.matches('/').count()),
                target.route
            )
        });
    }
    if let Some((_, _, Some(asset))) = catalog
        .references
        .get(&(page.id.as_str().into(), value.into()))
    {
        let target = catalog.pages.get(asset).ok_or_else(|| {
            anyhow::anyhow!("image link asset is not selected")
        })?;
        return Ok(format!(
            "{}{}",
            "../".repeat(page.route.matches('/').count()),
            target.route
        ));
    }
    if let Some((path, url, asset)) = catalog
        .references
        .get(&(page.id.as_str().into(), value.into())) &&
        asset.is_none() &&
        !catalog.sources.contains_key(path)
    {
        return Ok(url.clone());
    }
    if let Ok(url) = url::Url::parse(value) {
        ensure!(
            matches!(url.scheme(), "https" | "http" | "mailto"),
            "unsupported website link {value}"
        );
        // A link beneath the website's own location stays inside the website.
        if let Some(local) = catalog.bases.localize(&url) {
            let prefix = "../".repeat(page.route.matches('/').count());
            return Ok(if prefix.is_empty() && local.is_empty() {
                "./".into()
            } else {
                format!("{prefix}{local}")
            });
        }
        return Ok(
            sibling_link(&url, page, catalog).unwrap_or_else(|| value.into())
        );
    }
    let base = url::Url::parse(&format!(
        "https://grapharch.invalid/{}",
        page.source
    ))?;
    let url = base.join(value)?;
    ensure!(
        url.host_str() == Some("grapharch.invalid") && url.query().is_none(),
        "unsupported local documentation URL {value}"
    );
    let path = url.path().trim_start_matches('/');
    let target = if value.starts_with('#') || value.is_empty() {
        page
    } else if let Some(id) = catalog
        .sources
        .get(path)
        .or_else(|| catalog.routes.get(path))
        .or_else(|| {
            catalog
                .routes
                .get(&format!("{}/", path.trim_end_matches('/')))
        })
    {
        catalog.pages.get(id).unwrap()
    } else if let Some(links) = &catalog.source_links {
        // A repository file that is not a page, such as a configuration file
        // beside a README, links to the collected commit.
        return links
            .href(&page.source, value)
            .map_err(|error| anyhow::anyhow!("{}: {error:#}", page.source));
    } else {
        anyhow::bail!(
            "{}: unresolved documentation link {value} (source {path}); \
             declare ga:{} on the project to link repository files that are \
             not pages",
            page.source,
            crate::publication::source_link::SETTING
        )
    };
    let anchor = url.fragment();
    if let Some(anchor) = anchor.filter(|value| !value.is_empty()) {
        ensure!(
            !target.asset && target.headings.contains(anchor),
            "{}: unresolved heading {value} in {}",
            page.source,
            target.source
        );
    }
    if target.id == page.id && value.starts_with('#') {
        return Ok(value.into());
    }
    let prefix = "../".repeat(page.route.matches('/').count());
    Ok(format!(
        "{prefix}{}{}",
        target.route,
        anchor.map(|a| format!("#{a}")).unwrap_or_default()
    ))
}

/// A repository link to a page of a registered website: a sibling site's
/// page, or a page of this build when the repository is this site's own.
fn sibling_link(
    url: &url::Url,
    page: &Page,
    catalog: &Catalog,
) -> Option<String> {
    match catalog.siblings.destination(url)? {
        super::site_registry::Destination::Sibling(site) => Some(site.into()),
        super::site_registry::Destination::Local(source, anchor) => {
            let target = if source.is_empty() {
                catalog
                    .routes
                    .get("")
                    .and_then(|id| catalog.pages.get(id))?
            } else {
                catalog.pages.get(catalog.sources.get(&source)?)?
            };
            if let Some(anchor) = &anchor {
                target.headings.contains(anchor.as_str()).then_some(())?;
            }
            let prefix = "../".repeat(page.route.matches('/').count());
            let route = format!("{prefix}{}", target.route);
            let route = if route.is_empty() {
                "./".to_owned()
            } else {
                route
            };
            Some(match anchor {
                Some(anchor) => format!("{route}#{anchor}"),
                None => route,
            })
        },
    }
}
