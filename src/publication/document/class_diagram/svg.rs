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
            "<svg role=\"img\" aria-label=\"{} class neighborhood\" ",
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

/// Describe the retained visual labels without introducing a visible duplicate.
pub(crate) fn description(projection: &Projection) -> Result<String> {
    let document = roxmltree::Document::parse(&projection.svg)?;
    let mut description = format!(
        "UML class diagram for {}. Diagram labels: ",
        projection.diagram.focus.label
    );
    // bounded: labels in the validated, capped retained SVG only.
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
    {
        for text in node
            .descendants()
            .filter_map(|node| node.is_text().then(|| node.text()).flatten())
        {
            description.push_str(text);
        }
        description.push_str("; ");
    }
    if document.descendants().any(|node| {
        node.attribute("data-qualified-name") == Some("unknownDomain")
    }) {
        description.push_str(
            " The dotted rdfs:Resource source is a presentation placeholder: \
             no specific domain is declared in the selected inputs. It is not \
             an asserted domain or a claim that no more specific domain can \
             be entailed. ",
        );
    }
    if !super::qualified_count::collect(&projection.diagram).is_empty() {
        description.push_str(
            " Qualified counts count only property values in the named target \
             class; other values are not excluded. A zero qualified maximum \
             prohibits values in that target class, not other property \
             values. A zero qualified minimum imposes no restriction and is \
             omitted visually. ",
        );
    }
    if !super::nested_existence::collect(&projection.diagram).is_empty() {
        description.push_str(
            " The nested zero-minimum filler is unconstrained, but its outer \
             existential still requires a property value. A positive \
             qualified minimum or exact count on the same class and property \
             already guarantees that existence; otherwise the member retains \
             [1..*]. ",
        );
    }
    for edge in &projection.diagram.edges {
        description.push_str(&format!(
            " {} {} {}.",
            edge.from.label, edge.relation, edge.to.label
        ));
    }
    Ok(description)
}
