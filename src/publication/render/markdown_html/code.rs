//! Static code presentation; authored fences retain their language and example
//! role.
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

pub(super) fn events<'a>(
    input: Vec<Event<'a>>,
) -> anyhow::Result<Vec<Event<'a>>> {
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
        let mut source = String::new();
        for event in input.by_ref() {
            if matches!(event, Event::End(TagEnd::CodeBlock)) {
                break
            }
            if let Event::Text(text) = event {
                source.push_str(&text)
            }
        }
        let code = super::super::code::html(&source, language)?;
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
    Ok(output)
}

#[cfg(test)]
mod tests {

    #[test]
    fn requested_language_aliases_are_coloured_without_changing_source() {
        let cases = [
            (
                "python",
                "def greeting(name):\n    return f\"Hello 東京 {name}\"\n",
            ),
            ("py", "print(\"Bønds\")\n"),
            ("bash", "#!/bin/bash\nprintf '%s\\n' \"$HOME\"\n"),
            ("sh", "echo \"$value\"\n"),
            ("shell", "echo 'hello'\n"),
            ("javascript", "const value = `Hello ${name}`;\n"),
            ("js", "const value = true;\n"),
            ("node", "const value = true;\n"),
            ("nodejs", "const value = true;\n"),
            ("typescript", "const name: string = '東京';\n"),
            ("ts", "const count: number = 42;\n"),
            ("json", r#"{"key": true}"#),
            ("jsonld", r#"{"@id": "https://example.org/東京"}"#),
            (
                "json-ld",
                r#"{"@context": {"name": "https://schema.org/name"}}"#,
            ),
            (
                "sparql",
                "SELECT ?s $value WHERE { ?s <https://example.org/p> $value \
                 }\n",
            ),
            (
                "rq",
                "PREFIX ex: <https://example.org/>\nSELECT ?s WHERE { ?s ex:p \
                 \"東京\" }\n",
            ),
        ];
        for (language, source) in cases {
            let markdown = format!("```{language}\n{source}\n```\n");
            let mut html = String::new();
            pulldown_cmark::html::push_html(
                &mut html,
                super::events(pulldown_cmark::Parser::new(&markdown).collect())
                    .unwrap()
                    .into_iter(),
            );
            let document = scraper::Html::parse_fragment(&html);
            let code = document
                .select(&scraper::Selector::parse("code").unwrap())
                .next()
                .unwrap();
            assert_eq!(
                code.text().collect::<String>(),
                format!("{source}\n"),
                "{language}"
            );
            assert!(
                code.select(&scraper::Selector::parse("span").unwrap())
                    .next()
                    .is_some(),
                "{language} has no syntax colouring"
            );
        }
    }

    #[test]
    fn highlighting_preserves_unicode_escape_text_and_inert_markup() {
        for source in [
            r#"{"label": "Bønds 東京", "markup": "<script>& </code>", "escaped": "\\\""}"#,
            "{\"invalidEscape\": \"\\東京\", \"unfinished\": \"",
            "{\"value\": -1.25e+3, \"nothing\": null}\n",
        ] {
            let html = super::super::super::code::html(source, "json").unwrap();
            let document = scraper::Html::parse_fragment(&html);
            let code = document
                .select(&scraper::Selector::parse("code").unwrap())
                .next()
                .unwrap();
            assert_eq!(code.text().collect::<String>(), source);
            assert!(
                document
                    .select(&scraper::Selector::parse("script").unwrap())
                    .next()
                    .is_none()
            );
        }
    }
}
