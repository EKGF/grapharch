use {
    super::super::{
        content::letter,
        markup::{Markdown, html_escape},
    },
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Tag, TagEnd},
};

#[derive(Default)]
pub(super) struct Outline {
    sections:    [u32; 8],
    appendix:    u32,
    prefix:      Option<String>,
    levels:      [u32; 4],
    frontmatter: u8,
}

impl Outline {
    pub fn component(
        &mut self,
        kind: &str,
        title: &str,
        depth: usize,
    ) -> Result<String> {
        ensure!(
            (1..=8).contains(&depth),
            "component depth must be between one and eight"
        );
        self.levels = [0; 4];
        match kind {
            "abstract" => {
                ensure!(
                    self.frontmatter == 0,
                    "specification requires Abstract first, exactly once"
                );
                self.frontmatter = 1;
                self.prefix = None;
            },
            "status" => {
                ensure!(
                    self.frontmatter == 1,
                    "specification requires Status of This Document after \
                     Abstract, exactly once"
                );
                self.frontmatter = 2;
                self.prefix = None;
            },
            _ => {
                ensure!(
                    self.frontmatter == 2,
                    "specification requires Abstract and Status of This \
                     Document before its sections"
                );
                self.prefix = match kind {
                    "section" => {
                        ensure!(
                            self.sections[..depth - 1].iter().all(|n| *n > 0),
                            "component hierarchy skips a level"
                        );
                        self.sections[depth - 1] += 1;
                        self.sections[depth..].fill(0);
                        Some(
                            self.sections[..depth]
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join("."),
                        )
                    },
                    "appendix" => {
                        self.appendix += 1;
                        Some(letter(self.appendix))
                    },
                    _ => None,
                };
            },
        }
        Ok(match &self.prefix {
            Some(prefix) => format!("{prefix}. {title}"),
            None => title.to_owned(),
        })
    }

    pub fn heading_number(&mut self, depth: usize) -> Result<Option<String>> {
        ensure!(
            (1..=4).contains(&depth),
            "specification headings exceed HTML h6 depth"
        );
        let Some(prefix) = &self.prefix else {
            return Ok(None)
        };
        ensure!(
            self.levels[..depth - 1].iter().all(|n| *n > 0),
            "specification heading skips a level across fragments"
        );
        self.levels[depth - 1] += 1;
        self.levels[depth..].fill(0);
        Ok(Some(format!(
            "{prefix}.{}",
            self.levels[..depth]
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(".")
        )))
    }

    pub fn finish(&self) -> Result<()> {
        ensure!(
            self.frontmatter == 2,
            "specification requires Abstract and Status of This Document"
        );
        Ok(())
    }
}

pub(super) struct Heading {
    pub depth: usize,
    pub id:    String,
    pub text:  String,
}

pub(super) fn headings(markdown: &Markdown<'_>) -> Result<Vec<Heading>> {
    let mut headings = Vec::new();
    for (index, event) in markdown.events.iter().enumerate() {
        if let Event::Start(Tag::Heading { level, id, .. }) = event {
            let depth = markdown.depth(*level);
            ensure!(
                depth <= 4,
                "specification headings exceed HTML h6 depth"
            );
            let mut text = String::new();
            for part in &markdown.events[index + 1..] {
                match part {
                    Event::End(TagEnd::Heading(_)) => break,
                    Event::Text(part) | Event::Code(part) => {
                        text.push_str(part)
                    },
                    Event::SoftBreak | Event::HardBreak => text.push(' '),
                    _ => {},
                }
            }
            headings.push(Heading {
                depth,
                id: id
                    .as_ref()
                    .ok_or_else(|| {
                        anyhow::anyhow!("heading has no stable identity")
                    })?
                    .to_string(),
                text,
            });
        }
    }
    Ok(headings)
}

pub(super) fn markdown_html(
    markdown: &Markdown<'_>,
    outline: &mut Outline,
) -> Result<String> {
    let mut events = Vec::with_capacity(markdown.events.len());
    let mut current = None;
    for event in &markdown.events {
        match event {
            Event::Start(Tag::Heading { level, id, .. }) => {
                let depth = markdown.depth(*level);
                let number = outline
                    .heading_number(depth)?
                    .map(|n| format!("{n} "))
                    .unwrap_or_default();
                let id = id.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("heading has no stable identity")
                })?;
                let logical_level = depth + markdown.component_depth + 1;
                let level = logical_level.min(6);
                let aria = if logical_level > 6 {
                    format!(" role=\"heading\" aria-level=\"{logical_level}\"")
                } else {
                    String::new()
                };
                current = Some((level, id.to_string()));
                events.push(Event::Html(
                    format!(
                        "<h{level}{aria} id=\"{}\">{number}",
                        html_escape(id)
                    )
                    .into(),
                ));
            },
            Event::End(TagEnd::Heading(_)) => {
                let (level, id) = current.take().ok_or_else(|| {
                    anyhow::anyhow!("heading end without start")
                })?;
                events.push(Event::Html(
                    format!("{}</h{level}>", permalink(&id)).into(),
                ));
            },
            event => events.push(event.clone()),
        }
    }
    let mut output = String::new();
    pulldown_cmark::html::push_html(&mut output, events.into_iter());
    Ok(output)
}

pub(super) fn permalink(id: &str) -> String {
    format!(
        "<a class=\"self-link\" href=\"#{}\" aria-label=\"Link to this \
         section\">§</a>",
        html_escape(id)
    )
}

pub(super) struct Toc {
    depth: usize,
}
impl Toc {
    pub fn new() -> Self { Self { depth: 0 } }

    pub fn entry(
        &mut self,
        depth: usize,
        id: &str,
        label: &str,
    ) -> Result<String> {
        ensure!(
            depth <= self.depth + 1,
            "table of contents skips a heading level"
        );
        let mut html = String::new();
        if depth > self.depth {
            html.push_str("<ol>");
        } else {
            html.push_str("</li>");
            for _ in depth..self.depth {
                html.push_str("</ol></li>");
            }
        }
        self.depth = depth;
        html.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            html_escape(id),
            toc_label(label)
        ));
        Ok(html)
    }

    pub fn finish(&self) -> String { "</li></ol>".repeat(self.depth) }
}

fn toc_label(label: &str) -> String {
    if let Some((number, text)) = label.split_once(' ') &&
        number.chars().all(|c| {
            c.is_ascii_digit() || c == '.' || c.is_ascii_uppercase()
        })
    {
        return format!(
            "<span class=\"toc-number\">{}</span> <span \
             class=\"toc-label\">{}</span>",
            html_escape(number),
            html_escape(text)
        );
    }
    format!(
        "<span class=\"toc-label\">{}</span>",
        html_escape(label)
    )
}
