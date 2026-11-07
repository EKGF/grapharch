//! Semantic explicit destinations, independent of source HTML syntax.
pub(crate) const PREFIX: &str = "urn:grapharch:anchor:";
pub(crate) fn label(destination: &str) -> Option<&str> {
    destination
        .strip_prefix(PREFIX)
        .filter(|value| valid(value))
}
pub(crate) fn valid(value: &str) -> bool {
    !value.is_empty() &&
        value.len() <= 512 &&
        value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() ||
                matches!(byte, b'-' | b'_' | b'.' | b':')
        })
}
pub(crate) fn labels(body: &str) -> anyhow::Result<Vec<String>> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut labels = Vec::new(); // bounded: caller's 1 MiB page budget, at most 1024 anchors.
    let mut events =
        Parser::new_ext(body, crate::publication::markdown::options());
    while let Some(event) = events.next() {
        if let Event::Start(Tag::Link { dest_url, .. }) = event &&
            dest_url.starts_with(PREFIX)
        {
            let value = label(&dest_url)
                .ok_or_else(|| anyhow::anyhow!("invalid explicit anchor"))?;
            anyhow::ensure!(
                matches!(events.next(), Some(Event::End(TagEnd::Link))),
                "explicit anchor must have no link body"
            );
            anyhow::ensure!(
                labels.len() < 1024,
                "page exceeds 1024 explicit anchors"
            );
            labels.push(value.to_string());
        }
    }
    Ok(labels)
}
