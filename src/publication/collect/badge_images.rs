//! Status badge images, recorded as references without a retained asset.
use {
    super::source::FileInput,
    crate::publication::term,
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

/// The reference quads of a badge image, or `None` for any other image.
pub(super) fn reference(
    input: &FileInput,
    dest_url: &str,
) -> Option<Vec<Quad>> {
    let remote = url::Url::parse(dest_url).ok()?;
    if !crate::publication::reference::is_badge(&remote) {
        return None;
    }
    let reference =
        crate::publication::dataset::identity("markdown-image-reference", &[
            input.fragment_id().as_str(),
            dest_url,
        ]);
    Some(vec![
        input.quad(
            &input.fragment_id(),
            "reference",
            reference.clone(),
        ),
        input.quad(&reference, "type", term("ExternalReference")),
        input.quad(&reference, "referenceKind", term("BadgeLink")),
        input.quad(
            &reference,
            "url",
            Literal::new_simple_literal(remote.as_str()),
        ),
        Quad::new(
            reference,
            NamedNode::new_unchecked("http://purl.org/dc/terms/identifier"),
            Literal::new_simple_literal(dest_url),
            GraphName::DefaultGraph,
        ),
    ])
}
