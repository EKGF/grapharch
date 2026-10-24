use {
    super::model::Plan,
    anyhow::{Result, bail, ensure},
    pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd},
    std::collections::{HashMap, HashSet},
};

pub(super) struct Markdown<'a> {
    // bounded: one fragment, capped at 1 MiB before parsing; never the corpus.
    pub events: Vec<Event<'a>>,
    base_level: usize,
}

impl<'a> Markdown<'a> {
    pub async fn parse(
        body: &'a str,
        plan: &Plan,
        prefix: &str,
    ) -> Result<Self> {
        ensure!(
            body.len() <= 1024 * 1024,
            "fragment exceeds Markdown rendering limit"
        );
        let options = Options::ENABLE_TABLES |
            Options::ENABLE_STRIKETHROUGH |
            Options::ENABLE_TASKLISTS;
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
                Event::Start(Tag::Image { .. }) => {
                    bail!("Markdown images require collected asset support")
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
                        let resolved = plan.link(&original, prefix).await?;
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
        Ok(Self { events, base_level })
    }

    pub fn depth(&self, level: HeadingLevel) -> usize {
        level as usize - self.base_level + 1
    }

    pub fn html(&self) -> String {
        let events = self.events.iter().cloned().map(|event| {
            match event {
                Event::Start(Tag::Heading { level, id, classes, attrs }) => {
                    Event::Start(Tag::Heading {
                        level: shifted(self.depth(level)),
                        id,
                        classes,
                        attrs,
                    })
                },
                Event::End(TagEnd::Heading(level)) => {
                    Event::End(TagEnd::Heading(shifted(self.depth(level))))
                },
                event => event,
            }
        });
        let mut output = String::new();
        pulldown_cmark::html::push_html(&mut output, events);
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
