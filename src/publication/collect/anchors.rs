//! Normalize exact empty source anchors into retained semantic destinations.
use {
    anyhow::Result,
    pulldown_cmark::{Event, Parser},
};
pub(super) fn normalize(body: &str) -> Result<String> {
    let pattern =
        regex::Regex::new(r#"^<(span|a) id="([A-Za-z0-9_.:-]{1,128})">$"#)?;
    let events: Vec<_> =
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
            .collect(); // bounded: caller's 1 MiB source.
    let mut result = body.to_owned();
    for pair in events.windows(2).rev() {
        if let (Event::InlineHtml(open), Event::InlineHtml(close)) =
            (&pair[0].0, &pair[1].0) &&
            let Some(capture) = pattern.captures(open) &&
            close.as_ref() == format!("</{}>", &capture[1])
        {
            result.replace_range(
                pair[0].1.start..pair[1].1.end,
                &format!(
                    "[]({}{})",
                    crate::publication::anchor::PREFIX,
                    &capture[2]
                ),
            );
        }
    }
    crate::publication::anchor::labels(&result)?;
    Ok(result)
}
