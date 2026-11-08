//! Route and namespace a retained engine SVG without performing layout.
use {
    super::{Projection, projection},
    anyhow::Result,
    sha2::{Digest, Sha256},
};
pub(crate) fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(crate) fn safe_link(value: &str) -> Result<()> {
    anyhow::ensure!(
        !value.chars().any(char::is_control) && !value.contains('\\'),
        "invalid diagram link"
    );
    anyhow::ensure!(
        value.starts_with('#') ||
            value.starts_with("./") ||
            value.starts_with("../") ||
            value.starts_with("https://") ||
            value.starts_with("http://"),
        "unsupported class diagram link {value}"
    );
    Ok(())
}

pub(crate) fn draw(projection: &Projection, links: bool) -> Result<String> {
    let mut result = projection.svg.clone();
    for (index, node) in
        projection::nodes(&projection.diagram).values().enumerate()
    {
        let placeholder = format!("./grapharch-node/n{index}");
        for name in ["xlink:href", "href"] {
            let original = format!("{name}=\"{placeholder}\"");
            let replacement = if links {
                match &node.link {
                    Some(link) => {
                        safe_link(link)?;
                        format!("{name}=\"{}\"", xml(link))
                    },
                    None => String::new(),
                }
            } else {
                String::new()
            };
            result = result.replace(&original, &replacement);
        }
    }
    let namespace = format!(
        "owl-{}-",
        hex::encode(Sha256::digest(serde_json::to_vec(
            &projection.diagram
        )?))
    );
    let document = roxmltree::Document::parse(&result)?;
    let ids: Vec<String> = document
        .descendants()
        .filter_map(|node| node.attribute("id").map(str::to_owned))
        .collect(); // bounded: validated 256 KiB SVG.
    let style = document
        .root_element()
        .attribute("style")
        .map(str::to_owned);
    for id in ids {
        result = result
            .replace(
                &format!("id=\"{id}\""),
                &format!("id=\"{namespace}{id}\""),
            )
            .replace(
                &format!("url(#{id})"),
                &format!("url(#{namespace}{id})"),
            );
    }
    let responsive = "display:block;max-width:100%;height:auto";
    if let Some(style) = style {
        result = result.replacen(
            &format!("style=\"{style}\""),
            &format!("style=\"{responsive}\""),
            1,
        );
    } else {
        result = result.replacen(
            "<svg ",
            &format!("<svg style=\"{responsive}\" "),
            1,
        );
    }
    result = result.replacen(
        "<svg ",
        &format!(
            "<svg role=\"img\" aria-label=\"{} class neighborhood\" ",
            xml(&projection.diagram.focus.label)
        ),
        1,
    );
    Ok(result)
}
