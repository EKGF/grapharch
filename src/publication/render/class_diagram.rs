//! Render retained semantic diagram blocks; no source RDF or files are read.
use {
    crate::publication::document::class_diagram::{LANGUAGE, Projection, svg},
    anyhow::Result,
    pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd},
    std::collections::HashMap,
};

pub(super) fn resolve<'a>(
    events: Vec<Event<'a>>,
    mut link: impl FnMut(&str) -> Result<String>,
) -> Result<Vec<Event<'a>>> {
    let mut output = Vec::new(); // bounded: one capped fragment/page.
    let mut input = events.into_iter();
    let mut links: HashMap<String, String> = HashMap::new(); // bounded: node links within one capped fragment/page.
    while let Some(event) = input.next() {
        if !matches!(&event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if info.as_ref() == LANGUAGE)
        {
            output.push(event);
            continue;
        }
        let mut source = String::new();
        for event in input.by_ref() {
            match event {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(text) => source.push_str(&text),
                _ => anyhow::bail!("invalid class diagram content"),
            }
        }
        let mut projection = Projection::parse(&source)?;
        let diagram = &mut projection.diagram;
        for node in diagram.nodes_mut() {
            if let Some(original) = &node.link {
                let destination = match links.get(original) {
                    Some(value) => value.clone(),
                    None => {
                        let value = link(original)?;
                        links.insert(original.clone(), value.clone());
                        value
                    },
                };
                svg::safe_link(&destination)?;
                node.link = Some(destination);
            }
        }
        output.push(event);
        output.push(Event::Text(
            serde_json::to_string(&projection)?.into(),
        ));
        output.push(Event::End(TagEnd::CodeBlock));
    }
    Ok(output)
}
pub(super) fn html(source: &str) -> Result<String> {
    let projection = Projection::parse(source)?;
    let diagram = &projection.diagram;
    Ok(format!(
        "<figure class=\"owl-class-diagram\" \
         style=\"width:fit-content;max-width:100%;margin:1.25rem auto\"><div \
         class=\"class-diagram-scroll\" tabindex=\"0\" role=\"region\" \
         aria-label=\"Class relationship diagram; scroll horizontally when \
         needed\" style=\"overflow-x:auto;background:#fff;padding:0.75rem;\
         border-radius:0.4rem\">{}</div><figcaption>Class neighborhood: {} \
         {}</figcaption></figure>\n",
        svg::draw(&projection, true)?,
        svg::xml(&diagram.focus.label),
        super::diagram_notes::reference_html()
    ))
}
pub(super) fn typst(source: &str) -> Result<String> {
    let projection = Projection::parse(source)?;
    let diagram = &projection.diagram;
    let svg = svg::draw(&projection, false)?;
    crate::publication::image_assets::validate(
        svg.as_bytes(),
        "image/svg+xml",
    )?;
    let quote = super::markup::quote;
    let document = roxmltree::Document::parse(&svg)?;
    let bounds = document
        .root_element()
        .attribute("viewBox")
        .ok_or_else(|| anyhow::anyhow!("class diagram has no viewBox"))?;
    let mut dimensions = bounds.split_whitespace().skip(2);
    let width: f64 = dimensions
        .next()
        .ok_or_else(|| anyhow::anyhow!("class diagram has no width"))?
        .parse()?;
    let height: f64 = dimensions
        .next()
        .ok_or_else(|| anyhow::anyhow!("class diagram has no height"))?
        .parse()?;
    anyhow::ensure!(
        width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
        "invalid class diagram dimensions"
    );
    // Bound height and intrinsic size as well as available width. SVG pixels
    // become 0.75 PDF points, preserving the generator's font scale instead
    // of enlarging a compact class box merely to fill the text column.
    let mut result = format!(
        "#block(breakable: false)[#layout(size => image(bytes({}), format: \
         \"svg\", width: calc.min(size.width * {}, {}pt), alt: {}))\n",
        quote(&svg),
        if diagram.edges.is_empty() && diagram.shapes.is_empty() {
            "0.35"
        } else {
            "1.0"
        },
        (width * 0.75).min(420.0 * width / height),
        quote(&svg::description(&projection)?)
    );
    result.push_str(super::diagram_notes::reference_typst());
    result.push_str("]\n");
    Ok(result)
}

pub(super) fn validate_external(destination: &str) -> Result<()> {
    let url = url::Url::parse(destination)?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "unresolved diagram resource: {destination}"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
