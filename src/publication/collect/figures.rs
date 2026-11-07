//! Semantic figures retain image, caption, attribution and bounded size intent.
use {
    crate::publication::content_blocks::Boundary,
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    regex::Regex,
};
pub(super) fn normalize(body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "figure source exceeds 1 MiB"
    );
    let mut protected = Vec::new(); // bounded: a capped Markdown source.
    let mut start = None;
    for (event, range) in
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = start.take() {
                    protected.push(start..range.end);
                }
            },
            Event::Code(_) => protected.push(range),
            _ => {},
        }
    }
    let figures = Regex::new(r"(?is)<figure\b([^>]*)>(.*?)</figure\s*>")?;
    let caption =
        Regex::new(r"(?is)^(.*?)<figcaption\s*>(.*?)</figcaption\s*>\s*$")?;
    let width = Regex::new(r#"\s*\{\s*width="([0-9]+)"\s*\}\s*$"#)?;
    let mut output = String::new();
    let mut offset = 0;
    let mut count = 0;
    for capture in figures.captures_iter(body) {
        let matched = capture.get(0).unwrap();
        if protected
            .iter()
            .any(|range| range.contains(&matched.start()))
        {
            continue;
        }
        count += 1;
        ensure!(count <= 128, "figure count exceeds 128");
        ensure!(
            matches!(capture[1].trim(), "" | "markdown"),
            "figure supports only its Markdown content hint"
        );
        let parts = caption.captures(&capture[2]).ok_or_else(|| {
            anyhow::anyhow!("figure requires one trailing figcaption")
        })?;
        let mut image = parts[1].trim().to_string();
        let mut max_width_px = None;
        if let Some(capture) = width.captures(&image) {
            let pixels: u16 = capture[1].parse()?;
            ensure!(
                (1..=2048).contains(&pixels),
                "figure width must be 1–2048 pixels"
            );
            max_width_px = Some(pixels);
            image.truncate(capture.get(0).unwrap().start());
        }
        let mut images = 0;
        let mut inside_image = false;
        for event in Parser::new(&image) {
            match event {
                Event::Start(Tag::Image { .. }) => {
                    images += 1;
                    inside_image = true;
                },
                Event::End(TagEnd::Image) => inside_image = false,
                Event::Start(Tag::Paragraph) |
                Event::End(TagEnd::Paragraph) => {},
                Event::Text(_) | Event::Code(_) if inside_image => {},
                _ => {
                    anyhow::bail!("figure body must contain one Markdown image")
                },
            }
        }
        ensure!(
            images == 1,
            "figure body requires exactly one image"
        );
        let caption = parts[2].trim();
        ensure!(
            !caption.is_empty() && caption.len() <= 16 * 1024,
            "figure caption must contain 1–16384 bytes"
        );
        output.push_str(&body[offset..matched.start()]);
        output.push_str(&Boundary::Figure { max_width_px }.markdown()?);
        output.push_str(&image);
        output.push('\n');
        output.push_str(&Boundary::Caption.markdown()?);
        output.push_str(caption);
        output.push('\n');
        output.push_str(&Boundary::End.markdown()?);
        output.push_str(&Boundary::End.markdown()?);
        offset = matched.end();
    }
    output.push_str(&body[offset..]);
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::normalize;
    #[test]
    fn figures_reject_active_attributes_and_preserve_literal_examples() {
        let figure = "<figure markdown>![A](a.svg)<figcaption>A \
                      caption</figcaption></figure>";
        let code = format!("```html\n{figure}\n```");
        assert_eq!(normalize(&code).unwrap(), code);
        assert!(
            normalize(
                &figure.replace("figure markdown>", "figure onclick=\"x\"> ")
            )
            .is_err()
        );
        assert!(
            normalize(&figure.replace("![A](a.svg)", "ordinary prose"))
                .is_err()
        );
        assert!(
            normalize(
                &figure.replace("![A](a.svg)", "![A](a.svg){ width=\"9999\" }")
            )
            .is_err()
        );
    }
}
