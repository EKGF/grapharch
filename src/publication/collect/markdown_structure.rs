//! Identify logical sections and examples in one Markdown document.
use {
    super::source::FileInput,
    crate::publication::{dataset::identity, term},
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Quad},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    std::collections::HashMap,
};

pub(super) fn describe(
    input: &FileInput,
    page: &NamedNode,
    body: &str,
) -> Result<Vec<Quad>> {
    let events: Vec<_> =
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
            .collect();
    let mut sections = vec![(
        0usize,
        "Introduction".to_owned(),
        0usize,
        String::from("introduction"),
    )];
    let mut slugs = HashMap::new();
    for (index, (event, range)) in events.iter().enumerate() {
        if let Event::Start(Tag::Heading { level, .. }) = event {
            let mut title = String::new();
            for (event, _) in &events[index + 1..] {
                match event {
                    Event::End(TagEnd::Heading(_)) => break,
                    Event::Text(text) | Event::Code(text) => {
                        title.push_str(text)
                    },
                    _ => {},
                }
            }
            let base = super::page::slug(&title);
            let occurrence = slugs.entry(base.clone()).or_insert(0usize);
            let slug = if *occurrence == 0 {
                base
            } else {
                format!("{base}-{occurrence}")
            };
            *occurrence += 1;
            let start = if body[..range.start].trim().is_empty() {
                sections.clear();
                0
            } else {
                range.start
            };
            sections.push((start, title, *level as usize, slug));
        }
    }
    ensure!(
        sections.len() <= 1024,
        "Markdown document exceeds 1024-topic budget"
    );
    let mut output = Vec::new();
    let mut parents: Vec<(usize, NamedNode)> = Vec::new();
    for (index, (start, title, level, slug)) in sections.iter().enumerate() {
        let end = sections
            .get(index + 1)
            .map_or(body.len(), |section| section.0);
        let topic = identity("topic", &[
            page.as_str(),
            if *level == 0 { "preamble" } else { "heading" },
            slug,
        ]);
        let fragment = identity("topic-fragment", &[topic.as_str()]);
        let text = &body[*start..end];
        output.extend([
            input.quad(page, "topic", topic.clone()),
            input.quad(&topic, "type", term("Topic")),
            input.quad(&topic, "type", term("Component")),
            input.quad(&topic, "partOf", page.clone()),
            input.quad(&topic, "fragment", fragment.clone()),
            input.quad(
                &topic,
                "headingAnchor",
                Literal::new_simple_literal(slug),
            ),
            input.quad(&fragment, "type", term("Fragment")),
            input.quad(&fragment, "subject", topic.clone()),
            input.quad(&fragment, "partOf", page.clone()),
            input.quad(
                &fragment,
                "body",
                Literal::new_simple_literal(text),
            ),
            input.quad(
                &fragment,
                "format",
                Literal::new_simple_literal("text/markdown"),
            ),
            input.quad(&fragment, "order", Literal::from(index as u32)),
            input.quad(&fragment, "derivedFrom", input.fragment_id()),
            input.quad(
                &fragment,
                "contribution",
                identity("contribution", &[
                    input.project.as_str(),
                    input.collector.as_str(),
                    input.source()?,
                ]),
            ),
        ]);
        output.push(Quad::new(
            topic.clone(),
            NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            Literal::new_simple_literal(title),
            oxrdf::GraphName::DefaultGraph,
        ));
        while parents.last().is_some_and(|parent| parent.0 >= *level) {
            parents.pop();
        }
        if let Some((_, parent)) = parents.last() {
            output.push(input.quad(&topic, "parentTopic", parent.clone()));
        }
        parents.push((*level, topic.clone()));
        let mut example_index = 0;
        let mut code = None;
        for event in
            Parser::new_ext(text, crate::publication::markdown::options())
        {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(language) => {
                            language.into_string()
                        },
                        _ => String::new(),
                    };
                    code = Some((language, String::new()));
                },
                Event::Text(text) if code.is_some() => {
                    code.as_mut().unwrap().1.push_str(&text)
                },
                Event::End(TagEnd::CodeBlock) => {
                    if let Some((language, body)) = code.take() {
                        let example = identity("code-example", &[
                            topic.as_str(),
                            &example_index.to_string(),
                        ]);
                        let example_fragment =
                            identity("example-fragment", &[example.as_str()]);
                        example_index += 1;
                        output.extend([
                            input.quad(&topic, "example", example.clone()),
                            input.quad(&example, "type", term("Example")),
                            input.quad(&example, "partOf", topic.clone()),
                            input.quad(
                                &example,
                                "fragment",
                                example_fragment.clone(),
                            ),
                            input.quad(
                                &example_fragment,
                                "type",
                                term("Fragment"),
                            ),
                            input.quad(
                                &example_fragment,
                                "subject",
                                example.clone(),
                            ),
                            input.quad(
                                &example_fragment,
                                "body",
                                Literal::new_simple_literal(body),
                            ),
                            input.quad(
                                &example_fragment,
                                "format",
                                Literal::new_simple_literal("text/plain"),
                            ),
                            input.quad(
                                &example_fragment,
                                "derivedFrom",
                                input.fragment_id(),
                            ),
                            input.quad(
                                &example_fragment,
                                "contribution",
                                identity("contribution", &[
                                    input.project.as_str(),
                                    input.collector.as_str(),
                                    input.source()?,
                                ]),
                            ),
                            input.quad(
                                &example,
                                "language",
                                Literal::new_simple_literal(language),
                            ),
                            input.quad(
                                &example,
                                "derivedFrom",
                                input.fragment_id(),
                            ),
                        ]);
                    }
                },
                _ => {},
            }
        }
    }
    Ok(output)
}
