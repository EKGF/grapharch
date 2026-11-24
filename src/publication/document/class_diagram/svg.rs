//! Route and namespace a retained engine SVG without performing layout.
use {
    super::{Projection, plantuml_source},
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

/// Diagram nodes by the entity name the layout engine gives each box.
pub(crate) fn entities(
    diagram: &super::Diagram,
) -> std::collections::BTreeMap<String, &super::Node> {
    plantuml_source::PlantUmlSource::index(diagram)
        .into_values()
        .enumerate()
        .map(|(index, node)| (format!("n{index}"), node))
        .collect() // bounded: at most thirty-seven nodes.
}

pub(crate) fn draw(projection: &Projection, links: bool) -> Result<String> {
    let mut result = projection.svg.clone();
    for (index, node) in
        plantuml_source::PlantUmlSource::index(&projection.diagram)
            .values()
            .enumerate()
    {
        let placeholder = format!("./grapharch-node/n{index}");
        for name in ["xlink:title", "title"] {
            let replacement = if links {
                format!("{name}=\"{}\"", xml(&node.id))
            } else {
                String::new()
            };
            result = result
                .replace(&format!("{name}=\"{placeholder}\""), &replacement);
        }
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
    let responsive = if links {
        let width: f64 = document
            .root_element()
            .attribute("viewBox")
            .and_then(|value| value.split_whitespace().nth(2))
            .ok_or_else(|| {
                anyhow::anyhow!("class diagram has no viewBox width")
            })?
            .parse()?;
        anyhow::ensure!(
            width.is_finite() && width > 0.0,
            "invalid class diagram width"
        );
        format!("display:block;width:{width}px;max-width:none;height:auto")
    } else {
        String::from("display:block;max-width:100%;height:auto")
    };
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
            "<svg role=\"img\" aria-label=\"UML class diagram of {}\" ",
            xml(&projection.diagram.focus.label)
        ),
        1,
    );
    let opening = result
        .find("<svg ")
        .ok_or_else(|| anyhow::anyhow!("missing SVG root"))?;
    let content = opening +
        result[opening..]
            .find('>')
            .ok_or_else(|| anyhow::anyhow!("invalid SVG root"))? +
        1;
    result.insert_str(
        content,
        &format!("<desc>{}</desc>", xml(&description(projection)?)),
    );
    Ok(result)
}

/// The retained plain-language description of the drawn UML elements.
pub(crate) fn description(projection: &Projection) -> Result<String> {
    Ok(projection.description.clone())
}
