//! Shared identification of Mermaid fences; layout belongs to the document
//! stage.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{CodeBlockKind, Event, LinkType, Tag, TagEnd},
    sha2::{Digest, Sha256},
};

pub(crate) fn id(code: &str) -> String {
    format!(
        "urn:grapharch:mermaid:{}",
        hex::encode(Sha256::digest(code.as_bytes()))
    )
}

pub(crate) fn events<'a>(events: Vec<Event<'a>>) -> Result<Vec<Event<'a>>> {
    // bounded: one Markdown fragment, capped by its parser's byte budget.
    let mut output = Vec::new();
    let mut events = events.into_iter();
    while let Some(event) = events.next() {
        if matches!(&event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) if language.as_ref() == "mermaid")
        {
            let mut code = String::new();
            for event in events.by_ref() {
                match event {
                    Event::End(TagEnd::CodeBlock) => break,
                    Event::Text(text) => code.push_str(&text),
                    _ => anyhow::bail!("unexpected event in Mermaid fence"),
                }
                ensure!(
                    code.len() <= 32 * 1024,
                    "Mermaid diagram exceeds 32 KiB"
                );
            }
            output.extend([
                Event::Start(Tag::Paragraph),
                Event::Start(Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url:  id(&code).into(),
                    title:     "".into(),
                    id:        "".into(),
                }),
                Event::Text("Documentation diagram".into()),
                Event::End(TagEnd::Image),
                Event::End(TagEnd::Paragraph),
            ]);
        } else {
            output.push(event);
        }
    }
    Ok(output)
}
