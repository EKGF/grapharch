//! Semantic inline identifier labels retained independently of HTML styling.
pub(crate) fn label(destination: &str) -> Option<&str> {
    destination
        .strip_prefix("urn:grapharch:badge:")
        .filter(|value| {
            !value.is_empty() &&
                value.len() <= 16 &&
                value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}
