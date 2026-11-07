//! Exact source icon tokens become decorative builtin image references.
use {
    crate::publication::icon::Icon,
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    regex::Regex,
};
pub(super) fn normalize(body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "inline icon source exceeds 1 MiB"
    );
    let pattern = Regex::new(
        r":((?:material|octicons)-[a-z0-9-]+):(?:[ \t]*\{[ \t]*\.lg[ \t]*\})?",
    )?;
    let mut output = String::new();
    let mut offset = 0;
    let mut count = 0;
    let mut code = false;
    for (event, range) in
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                code = true;
                continue
            },
            Event::End(TagEnd::CodeBlock) => {
                code = false;
                continue
            },
            Event::Text(_) if !code => {},
            _ => continue,
        }
        for capture in pattern.captures_iter(&body[range.clone()]) {
            count += 1;
            ensure!(count <= 128, "inline icons exceed 128 per source");
            let matched = capture.get(0).unwrap();
            let start = range.start + matched.start();
            let end = range.start + matched.end();
            let icon =
                Icon::from_source(&capture[1], matched.as_str().contains('{'))?;
            output.push_str(&body[offset..start]);
            output.push_str(&format!("![]({})", icon.uri()));
            offset = end;
        }
    }
    output.push_str(&body[offset..]);
    Ok(output)
}
#[cfg(test)]
mod tests {
    use {
        super::normalize,
        pulldown_cmark::{Event, Parser, Tag},
    };
    #[test]
    fn icons_inside_links_keep_labels_and_literal_examples() {
        let input = ":material-cached:{ .lg } \
                     __Deploy__\n\n[:octicons-arrow-right-24: Learn \
                     more](next.md)";
        let output = normalize(input).unwrap();
        assert!(output.contains("__Deploy__"));
        assert!(output.contains("Learn more"));
        assert_eq!(
            Parser::new(&output)
                .filter(|event| {
                    matches!(event, Event::Start(Tag::Image { .. }))
                })
                .count(),
            2
        );
        assert_eq!(
            Parser::new(&output)
                .filter(|event| matches!(event, Event::Start(Tag::Link { .. })))
                .count(),
            1
        );
        let code = format!("```md\n{input}\n```");
        assert_eq!(normalize(&code).unwrap(), code);
        assert!(normalize(":material-unknown:").is_err());
    }
}
