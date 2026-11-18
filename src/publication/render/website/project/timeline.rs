//! A timeline section's ordered list, presented as a vertical rail.
//!
//! Each list item opens with a paragraph `Status — when`, such as
//! `Done — 2024`. The status names one of the page's declared timeline
//! statuses, which supplies the item's icon and tone; the remainder is shown
//! as its date. A following paragraph that is wholly strong text is the item's
//! title.
use {
    super::presentation::Marker,
    crate::publication::render::markup::html_escape as esc,
    anyhow::{Context, Result, bail, ensure},
    scraper::ElementRef,
    std::collections::BTreeMap,
};

/// Separates an item's status from its date.
const SEPARATOR: &str = " — ";

pub(super) fn html(
    list: ElementRef,
    statuses: &BTreeMap<String, Marker>,
) -> Result<String> {
    let mut output = String::from("<ol class=\"project-timeline\">");
    for item in list.child_elements() {
        ensure!(
            item.value().name() == "li",
            "timeline list contains a non-item element"
        );
        output.push_str(&entry(item, statuses)?);
    }
    output.push_str("</ol>");
    Ok(output)
}

fn entry(
    item: ElementRef,
    statuses: &BTreeMap<String, Marker>,
) -> Result<String> {
    let mut paragraphs = item.child_elements();
    let Some(first) = paragraphs
        .next()
        .filter(|first| first.value().name() == "p")
    else {
        bail!(
            "a timeline item must open with a `Status{SEPARATOR}when` \
             paragraph; separate the item's paragraphs with blank lines"
        )
    };
    let heading = first.text().collect::<String>();
    let (status, when) = heading
        .split_once(SEPARATOR)
        .map(|(status, when)| (status.trim(), when.trim()))
        .filter(|(status, when)| !status.is_empty() && !when.is_empty())
        .with_context(|| {
            format!("timeline item {heading:?} is not `Status{SEPARATOR}when`")
        })?;
    let marker = statuses.get(status).with_context(|| {
        format!("timeline status {status:?} is not declared")
    })?;
    let mut content = format!(
        "<p class=\"project-timeline-meta\"><span class=\"project-badge \
         project-badge-{tone}\">{}</span><span \
         class=\"project-timeline-date\">{}</span></p>",
        esc(status),
        esc(when),
        tone = marker.tone.class(),
    );
    for paragraph in paragraphs {
        let html = paragraph.html();
        content.push_str(
            &if title(paragraph) {
                html.replacen("<p>", "<p class=\"project-timeline-title\">", 1)
            } else {
                html
            },
        );
    }
    Ok(format!(
        "<li class=\"project-timeline-item\">{}<div \
         class=\"project-timeline-content\">{content}</div></li>",
        marker.html()
    ))
}

/// A paragraph that is wholly one strong span.
fn title(paragraph: ElementRef) -> bool {
    let mut children = paragraph.child_elements();
    paragraph.value().name() == "p" &&
        matches!(
            (children.next(), children.next()),
            (Some(strong), None) if strong.value().name() == "strong" &&
                strong.text().collect::<String>().trim() ==
                    paragraph.text().collect::<String>().trim()
        )
}
