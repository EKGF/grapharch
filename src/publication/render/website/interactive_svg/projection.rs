//! Project validated SVG navigation without changing its retained original.
use {
    anyhow::{Result, ensure},
    std::{collections::BTreeSet, ops::Range},
};
struct Edit {
    range: Range<usize>,
    value: String,
}
pub(super) struct Prepared {
    pub bytes:  Vec<u8>,
    pub aspect: String,
}
pub(super) fn project(
    bytes: &[u8],
    resolve: impl Fn(&str) -> Result<String>,
) -> Result<Prepared> {
    crate::publication::image_assets::validate(bytes, "image/svg+xml")?;
    let source = std::str::from_utf8(bytes)?;
    let document = roxmltree::Document::parse(source)?;
    let root = document.root_element();
    let (width, height) = if let Some(view_box) = root.attribute("viewBox") {
        let mut fields = view_box
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|v| !v.is_empty());
        let mut values = [0.0f64; 4];
        for value in &mut values {
            *value = fields
                .next()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "interactive SVG requires a four-number viewBox"
                    )
                })?
                .parse()?;
            ensure!(
                value.is_finite(),
                "SVG viewport numbers must be finite"
            );
        }
        ensure!(
            fields.next().is_none(),
            "interactive SVG requires a four-number viewBox"
        );
        (values[2], values[3])
    } else {
        let size = |name| -> Result<f64> {
            Ok(root
                .attribute(name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "interactive SVG requires viewBox or numeric \
                         width/height"
                    )
                })?
                .strip_suffix("px")
                .unwrap_or(root.attribute(name).expect("checked attribute"))
                .parse()?)
        };
        (size("width")?, size("height")?)
    };
    ensure!(
        width.is_finite() &&
            height.is_finite() &&
            (1.0..=8192.0).contains(&width) &&
            (1.0..=8192.0).contains(&height),
        "interactive SVG viewport exceeds dimension budget"
    );
    let ids: BTreeSet<_> = document
        .descendants()
        .filter_map(|node| node.attribute("id"))
        .collect(); // bounded: validated SVG has at most 4096 elements.
    let mut edits = Vec::new(); // bounded: at most three edits per validated SVG element.
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "a")
    {
        let mut top = false;
        for attribute in node
            .attributes()
            .filter(|attribute| attribute.name() == "href")
        {
            if let Some(anchor) = attribute.value().strip_prefix('#') {
                ensure!(
                    ids.contains(anchor),
                    "unresolved SVG-local anchor {}",
                    attribute.value()
                );
                continue;
            }
            let value = resolve(attribute.value())?;
            edits.push(Edit {
                range: attribute.range_value(),
                value: super::super::super::markup::html_escape(&value),
            });
            top = true;
        }
        if top {
            if let Some(attribute) = node
                .attributes()
                .find(|attribute| attribute.name() == "target")
            {
                edits.push(Edit {
                    range: attribute.range_value(),
                    value: "_top".into(),
                });
            } else {
                let end = node
                    .attributes()
                    .map(|a| a.range().end)
                    .max()
                    .unwrap_or(node.range().start);
                let end = end +
                    source[end..node.range().end].find('>').ok_or_else(
                        || anyhow::anyhow!("SVG anchor has no start-tag end"),
                    )?;
                edits.push(Edit {
                    range: end..end,
                    value: " target=\"_top\"".into(),
                });
            }
        }
    }
    edits.sort_by_key(|edit| edit.range.start);
    let mut output = String::with_capacity(source.len());
    let mut offset = 0;
    for edit in edits {
        ensure!(
            edit.range.start >= offset,
            "overlapping SVG projection edits"
        );
        output.push_str(&source[offset..edit.range.start]);
        output.push_str(&edit.value);
        offset = edit.range.end;
    }
    output.push_str(&source[offset..]);
    ensure!(
        output.len() <= crate::publication::image_assets::MAX_IMAGE_BYTES,
        "projected SVG exceeds byte budget"
    );
    crate::publication::image_assets::validate(
        output.as_bytes(),
        "image/svg+xml",
    )?;
    Ok(Prepared {
        bytes:  output.into_bytes(),
        aspect: format!("{width} / {height}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projected_links_keep_quoted_attributes_and_reject_unsafe_or_unknown_targets()
    -> Result<()> {
        let source = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><a href="/page/" title="a > b"><text>Go</text></a></svg>"##;
        let projected = project(source, |value| {
            assert_eq!(value, "/page/");
            Ok("../page/#topic".into())
        })?;
        let text = std::str::from_utf8(&projected.bytes)?;
        assert!(text.contains("title=\"a > b\" target=\"_top\""));
        assert_eq!(projected.aspect, "100 / 50");
        assert!(
            project(
                source,
                |_| Ok("https://example.org/remote".into())
            )
            .is_err()
        );
        let missing = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><a href="#missing">Go</a></svg>"##;
        assert!(
            project(missing, |_| {
                panic!("SVG-local fragments stay local")
            })
            .is_err()
        );
        Ok(())
    }
}
