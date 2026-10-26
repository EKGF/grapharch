use {
    super::markup::{Markdown, quote},
    anyhow::{Result, bail},
    pulldown_cmark::{Alignment, CodeBlockKind, Event, Tag, TagEnd},
};

pub(super) fn convert(markdown: &Markdown<'_>) -> Result<String> {
    let mut output = String::new();
    let mut code = None;
    let mut heading = None;
    let mut table_head = false;
    let mut in_image = false;
    for (index, event) in markdown.events.iter().enumerate() {
        if in_image {
            if matches!(event, Event::End(TagEnd::Image)) {
                in_image = false;
            }
            continue;
        }
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Image { dest_url, .. } => {
                        let mut alt = String::new();
                        for event in &markdown.events[index + 1..] {
                            match event {
                                Event::End(TagEnd::Image) => break,
                                Event::Text(text) | Event::Code(text) => {
                                    alt.push_str(text)
                                },
                                Event::SoftBreak | Event::HardBreak => {
                                    alt.push(' ')
                                },
                                _ => {},
                            }
                        }
                        output.push_str(&format!(
                            "#image({}, alt: {})",
                            quote(dest_url),
                            quote(&alt)
                        ));
                        in_image = true;
                    },
                    Tag::Paragraph => {
                        output.push_str("#block(breakable: true)[")
                    },
                    Tag::Heading { level, id, .. } => {
                        heading = id.as_ref().map(ToString::to_string);
                        output.push_str(&format!(
                            "#heading(level: {})[",
                            markdown.depth(*level) + markdown.component_depth
                        ));
                    },
                    Tag::BlockQuote(kind) => {
                        if let Some(kind) = kind {
                            let label = super::callouts::label(*kind);
                            output.push_str(&format!(
                                "#ga-callout({}, kind: {})[",
                                quote(label),
                                quote(&label.to_ascii_lowercase())
                            ));
                        } else {
                            output.push_str("#quote(block: true)[");
                        }
                    },
                    Tag::CodeBlock(kind) => {
                        let lang = match kind {
                            CodeBlockKind::Fenced(language) => {
                                language.split_whitespace().next().unwrap_or("")
                            },
                            CodeBlockKind::Indented => "",
                        };
                        code = Some(String::new());
                        output.push_str(&format!(
                            "#raw(block: true, lang: {}, ",
                            quote(lang)
                        ));
                    },
                    Tag::List(None) => output.push_str("#list("),
                    Tag::List(Some(start)) => {
                        output.push_str(&format!("#enum(start: {start}, "))
                    },
                    Tag::Item => output.push('['),
                    Tag::Emphasis => output.push_str("#emph["),
                    Tag::Strong => output.push_str("#strong["),
                    Tag::Strikethrough => output.push_str("#strike["),
                    Tag::Link { dest_url, .. } => {
                        let destination =
                            if let Some(label) = dest_url.strip_prefix('#') {
                                format!("label({})", quote(label))
                            } else {
                                quote(dest_url)
                            };
                        output.push_str(&format!("#link({destination})["));
                    },
                    Tag::Table(columns) => {
                        let mut widths = columns
                            .iter()
                            .map(|alignment| {
                                if *alignment == Alignment::Right {
                                    "auto"
                                } else {
                                    "1fr"
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        if columns.len() == 1 {
                            widths.push(',');
                        }
                        let alignments = columns
                            .iter()
                            .map(|alignment| {
                                match alignment {
                                    Alignment::Right => "right",
                                    Alignment::Center => "center",
                                    _ => "left",
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        output.push_str(&format!(
                            "#ga-table(columns: ({widths}), align: \
                             ({alignments}), "
                        ));
                    },
                    Tag::TableHead => {
                        table_head = true;
                        output.push_str("table.header(");
                    },
                    Tag::TableRow => {},
                    Tag::TableCell => {
                        output.push_str(
                            if table_head { "[#strong[" } else { "[" },
                        )
                    },
                    other => {
                        bail!(
                            "unsupported Markdown construct in Typst: \
                             {other:?}"
                        )
                    },
                }
            },
            Event::End(tag) => {
                match tag {
                    TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
                        output.push_str("]\n")
                    },
                    TagEnd::Heading(_) => {
                        output.push(']');
                        if let Some(id) = heading.take() {
                            output.push_str(&format!("#label({})", quote(&id)));
                        }
                        output.push('\n');
                    },
                    TagEnd::CodeBlock => {
                        output.push_str(&quote(&code.take().ok_or_else(
                            || anyhow::anyhow!("unbalanced code block"),
                        )?));
                        output.push_str(")\n");
                    },
                    TagEnd::List(_) | TagEnd::Table => output.push_str(")\n"),
                    TagEnd::Item => output.push_str("],"),
                    TagEnd::TableCell => {
                        output.push_str(if table_head { "]]," } else { "]," })
                    },
                    TagEnd::TableHead => {
                        table_head = false;
                        output.push_str("),");
                    },
                    TagEnd::TableRow => {},
                    TagEnd::Emphasis |
                    TagEnd::Strong |
                    TagEnd::Strikethrough |
                    TagEnd::Link => output.push(']'),
                    other => {
                        bail!("unsupported Markdown ending in Typst: {other:?}")
                    },
                }
            },
            Event::Text(text) => {
                if let Some(code) = &mut code {
                    code.push_str(text);
                } else {
                    output.push_str(&format!("#text({})", quote(text)));
                }
            },
            Event::Code(text) => {
                output.push_str(&format!("#raw({})", quote(text)))
            },
            Event::SoftBreak => output.push(' '),
            Event::HardBreak => output.push_str("#linebreak()\n"),
            Event::Rule => {
                output.push_str("#line(length: 100%, stroke: 0.5pt)\n")
            },
            Event::TaskListMarker(checked) => {
                output.push_str(
                    if *checked {
                        "#text(\"[x] \")"
                    } else {
                        "#text(\"[ ] \")"
                    },
                )
            },
            other => bail!("unsupported Markdown event in Typst: {other:?}"),
        }
    }
    Ok(output)
}
