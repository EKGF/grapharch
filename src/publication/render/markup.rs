use {
    super::model::Plan,
    anyhow::{Result, bail, ensure},
    pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd},
    std::collections::{HashMap, HashSet},
};

pub(super) struct Markdown<'a> {
    // bounded: one fragment, capped at 1 MiB before parsing; never the corpus.
    pub events:          Vec<Event<'a>>,
    base_level:          usize,
    heading_depth:       usize,
    pub component_depth: usize,
}

impl<'a> Markdown<'a> {
    pub async fn parse(
        body: &'a str,
        plan: &Plan,
        prefix: &str,
        fragment: &oxrdf::NamedNode,
        component_depth: usize,
    ) -> Result<Self> {
        ensure!(
            body.len() <= 1024 * 1024,
            "fragment exceeds Markdown rendering limit"
        );
        let options = crate::publication::markdown::options();
        // bounded: maps and events refer only to this capped fragment.
        let mut events: Vec<_> = Parser::new_ext(body, options).collect();
        let base_level = events
            .iter()
            .filter_map(|event| {
                match event {
                    Event::Start(Tag::Heading { level, .. }) => {
                        Some(*level as usize)
                    },
                    _ => None,
                }
            })
            .min()
            .unwrap_or(1);
        let heading_depth =
            match plan.source.optional(fragment, "headingDepth").await? {
                None => 1,
                Some(oxrdf::Term::Literal(value)) => {
                    ensure!(
                        value.datatype().as_str() ==
                            "http://www.w3.org/2001/XMLSchema#integer",
                        "fragment headingDepth must have datatype xsd:integer"
                    );
                    value.value().parse::<usize>()?
                },
                Some(_) => bail!("fragment headingDepth must be an integer"),
            };
        ensure!(
            (1..=4).contains(&heading_depth),
            "fragment headingDepth must be between 1 and 4"
        );
        let mut previous_depth = 0;
        for event in &events {
            if let Event::Start(Tag::Heading { level, .. }) = event {
                let depth = *level as usize - base_level + 1;
                ensure!(
                    depth <= previous_depth + 1,
                    "Markdown headings skip an intermediate level"
                );
                previous_depth = depth;
            }
        }
        let mut headings = HashMap::new();
        let mut heading_ids = HashSet::new();
        let mut links = HashMap::new();
        for index in 0..events.len() {
            match &events[index] {
                Event::Html(_) |
                Event::InlineHtml(_) |
                Event::Start(Tag::HtmlBlock) => {
                    bail!("raw HTML is not supported in shared Markdown")
                },
                Event::Start(Tag::Image { dest_url, .. }) => {
                    let mut alt = String::new();
                    for part in &events[index + 1..] {
                        match part {
                            Event::End(TagEnd::Image) => break,
                            Event::Text(text) | Event::Code(text) => {
                                alt.push_str(text)
                            },
                            Event::SoftBreak | Event::HardBreak => {
                                alt.push(' ')
                            },
                            Event::Start(Tag::Image { .. }) => {
                                bail!("nested Markdown images are unsupported")
                            },
                            _ => {},
                        }
                    }
                    ensure!(
                        !alt.trim().is_empty() && alt.len() <= 1024,
                        "Markdown image alternative text must be nonempty and \
                         at most 1024 bytes"
                    );
                    let path =
                        plan.images.image(fragment, dest_url)?.to_owned();
                    if let Event::Start(Tag::Image { dest_url, .. }) =
                        &mut events[index]
                    {
                        *dest_url = path.into();
                    }
                },
                Event::Start(Tag::Table(columns)) => {
                    ensure!(
                        columns.len() <= 32,
                        "table exceeds 32-column rendering limit"
                    )
                },
                Event::Start(Tag::Heading { .. }) => {
                    let mut text = String::new();
                    for event in &events[index + 1..] {
                        match event {
                            Event::End(TagEnd::Heading(_)) => break,
                            Event::Text(part) | Event::Code(part) => {
                                text.push_str(part)
                            },
                            _ => {},
                        }
                    }
                    let slug = slug(&text);
                    let occurrence =
                        headings.entry(slug.clone()).or_insert(0usize);
                    let suffix = if *occurrence == 0 {
                        slug
                    } else {
                        format!("{slug}-{occurrence}")
                    };
                    *occurrence += 1;
                    if let Event::Start(Tag::Heading { id, .. }) =
                        &mut events[index]
                    {
                        let label = format!("{prefix}-h-{suffix}");
                        heading_ids.insert(label.clone());
                        *id = Some(label.into());
                    }
                },
                Event::Start(Tag::Link { dest_url, .. }) => {
                    let original = dest_url.to_string();
                    let resolved = if let Some(resolved) = links.get(&original)
                    {
                        resolved
                    } else {
                        let resolved =
                            match plan.images.link(fragment, &original) {
                                Some(url) => url.to_owned(),
                                None => plan.link(&original, prefix).await?,
                            };
                        links.entry(original).or_insert(resolved)
                    }
                    .clone();
                    if let Event::Start(Tag::Link { dest_url, .. }) =
                        &mut events[index]
                    {
                        *dest_url = resolved.into();
                    }
                },
                _ => {},
            }
        }
        for (original, resolved) in links {
            if original.starts_with('#') {
                ensure!(
                    resolved == format!("#{prefix}") ||
                        heading_ids
                            .contains(resolved.trim_start_matches('#')),
                    "unresolved local heading reference {original}"
                );
            }
        }
        Ok(Self { events, base_level, heading_depth, component_depth })
    }

    pub fn depth(&self, level: HeadingLevel) -> usize {
        level as usize - self.base_level + self.heading_depth
    }

    pub fn html(&self) -> String {
        let events = self.events.iter().cloned().map(|event| {
            match event {
                Event::Start(Tag::Heading { level, id, classes, attrs }) => {
                    let logical = self.depth(level) + self.component_depth + 1;
                    if logical > 6 {
                        let (_, aria) = heading_level(logical);
                        return Event::Html(
                            format!(
                                "<h6{aria} id=\"{}\">",
                                html_escape(id.as_deref().unwrap_or_default())
                            )
                            .into(),
                        );
                    }
                    Event::Start(Tag::Heading {
                        level: shifted(
                            self.depth(level) + self.component_depth - 1,
                        ),
                        id,
                        classes,
                        attrs,
                    })
                },
                Event::End(TagEnd::Heading(level)) => {
                    Event::End(TagEnd::Heading(shifted(
                        self.depth(level) + self.component_depth - 1,
                    )))
                },
                event => event,
            }
        });
        let mut output = String::new();
        pulldown_cmark::html::push_html(
            &mut output,
            super::markdown_html::events(events),
        );
        output
    }
}

fn shifted(depth: usize) -> HeadingLevel {
    match depth {
        1 => HeadingLevel::H3,
        2 => HeadingLevel::H4,
        3 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

fn slug(text: &str) -> String {
    let mut output = String::new();
    for character in text.to_lowercase().chars() {
        if character.is_alphanumeric() {
            output.push(character);
        } else if (character.is_whitespace() || character == '-') &&
            !output.ends_with('-')
        {
            output.push('-');
        }
    }
    if output.trim_matches('-').is_empty() {
        "heading".into()
    } else {
        output.trim_matches('-').to_owned()
    }
}

pub(super) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

/// HTML has six heading tags; ARIA preserves deeper semantic book levels.
pub(super) fn heading_level(logical: usize) -> (usize, String) {
    let attributes = if logical > 6 {
        format!(" role=\"heading\" aria-level=\"{logical}\"")
    } else {
        String::new()
    };
    (logical.min(6), attributes)
}
