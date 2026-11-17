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

/// The explicit destination of one subject documented inside a larger
/// fragment, such as a row of a reference table. Its label is derived from
/// the subject IRI, so a fragment's `ga:rowSubject` statements locate the
/// rows that its Markdown anchors.
pub(crate) struct SubjectAnchor(String);
impl SubjectAnchor {
    pub(crate) fn new(subject: &oxrdf::NamedNode) -> Self {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(subject.as_str().as_bytes());
        Self(format!("row-{}", hex::encode(&digest[..12])))
    }

    /// The anchor label, unique to the subject.
    pub(crate) fn label(&self) -> &str { &self.0 }

    /// The empty Markdown link that places this destination.
    pub(crate) fn markdown(&self) -> String {
        format!("[]({PREFIX}{})", self.0)
    }
}
