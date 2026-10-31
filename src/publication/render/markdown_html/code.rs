//! Static code presentation; authored fences retain their language and example
//! role.
use {
    crate::publication::render::markup::html_escape,
    pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd},
};

pub(super) fn events<'a>(input: Vec<Event<'a>>) -> Vec<Event<'a>> {
    // bounded: input and output represent a single capped Markdown fragment.
    let mut output = Vec::new();
    let mut input = input.into_iter();
    while let Some(event) = input.next() {
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = &event
        else {
            output.push(event);
            continue;
        };
        let mut fields = info.split_whitespace();
        let language = fields.next().unwrap_or("");
        let example = fields.any(|field| field == "example");
        if !example && !matches!(language, "json" | "jsonld" | "json-ld") {
            output.push(event);
            continue;
        }
        let mut source = String::new();
        for event in input.by_ref() {
            if matches!(event, Event::End(TagEnd::CodeBlock)) {
                break
            }
            if let Event::Text(text) = event {
                source.push_str(&text)
            }
        }
        let body = if matches!(language, "json" | "jsonld" | "json-ld") {
            json(&source)
        } else {
            html_escape(&source)
        };
        let code = format!(
            "<pre><code class=\"language-{}\">{body}</code></pre>",
            html_escape(language)
        );
        let html = if example {
            let caption = "<figure class=\"example code-example\"><figcaption \
                           class=\"marker\">Example <span \
                           class=\"example-number\"></span></figcaption>";
            format!("{caption}{code}</figure>\n")
        } else {
            code
        };
        output.push(Event::Html(html.into()));
    }
    output
}

fn json(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let class = match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' => {
                            index += 1;
                            if index < bytes.len() {
                                index += source[index..]
                                    .chars()
                                    .next()
                                    .unwrap()
                                    .len_utf8();
                            }
                        },
                        b'"' => {
                            index += 1;
                            break
                        },
                        _ => {
                            index += source[index..]
                                .chars()
                                .next()
                                .unwrap()
                                .len_utf8()
                        },
                    }
                }
                if source[index..].trim_start().starts_with(':') {
                    "json-key"
                } else {
                    "json-string"
                }
            },
            b'0'..=b'9' | b'-' => {
                index += 1;
                while index < bytes.len() &&
                    matches!(
                        bytes[index],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                    )
                {
                    index += 1
                }
                "json-number"
            },
            b't' | b'f' | b'n' => {
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_alphabetic()
                {
                    index += 1
                }
                if matches!(&source[start..index], "true" | "false" | "null") {
                    "json-keyword"
                } else {
                    ""
                }
            },
            _ => {
                index += source[index..].chars().next().unwrap().len_utf8();
                ""
            },
        };
        let text = html_escape(&source[start..index]);
        if class.is_empty() {
            output.push_str(&text)
        } else {
            output.push_str(&format!("<span class=\"{class}\">{text}</span>"))
        }
    }
    output
}
