//! Render retained semantic diagram blocks; no source RDF or files are read.
use {
    crate::publication::document::class_diagram::{
        LANGUAGE,
        Node,
        Projection,
        svg,
    },
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
    let mut html = format!(
        "<figure class=\"owl-class-diagram\" style=\"margin:1.25rem 0\"><div \
         tabindex=\"0\" role=\"region\" aria-label=\"Class relationship \
         diagram\" style=\"overflow-x:auto;background:#fff;padding:0.75rem;\
         border-radius:0.4rem\">{}</div><figcaption>Class neighborhood: \
         {}</figcaption><details><summary>Relationships as text</summary><ul>",
        svg::draw(&projection, true)?,
        svg::xml(&diagram.focus.label)
    );
    if diagram.edges.is_empty() {
        html.push_str(&format!(
            "<li>{}</li>",
            node_html(&diagram.focus)?
        ));
    }
    for edge in &diagram.edges {
        html.push_str(&format!(
            "<li>{} — {} → {}</li>",
            node_html(&edge.from)?,
            svg::xml(&edge.relation),
            node_html(&edge.to)?
        ));
    }
    html.push_str("</ul></details></figure>\n");
    Ok(html)
}
fn node_html(node: &Node) -> Result<String> {
    if let Some(link) = &node.link {
        svg::safe_link(link)?;
        Ok(format!(
            "<a href=\"{}\">{}</a>",
            svg::xml(link),
            svg::xml(&node.label)
        ))
    } else {
        Ok(svg::xml(&node.label))
    }
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
    let mut result = format!(
        "#block(breakable: false)[#image(bytes({}), format: \"svg\", width: \
         {}, alt: {})]\n",
        quote(&svg),
        if diagram.edges.is_empty() {
            "35%"
        } else {
            "100%"
        },
        quote(&format!(
            "Class neighborhood for {}",
            diagram.focus.label
        ))
    );
    // Text links remain accessible and clickable in PDF, where SVG hyperlink
    // annotations are not retained by the image decoder.
    for edge in &diagram.edges {
        result.push_str(&format!(
            "#par(text(size: 8pt)[{} #text({}) {}])\n",
            node_typst(&edge.from),
            quote(&format!("— {} →", edge.relation)),
            node_typst(&edge.to)
        ));
    }
    Ok(result)
}
fn node_typst(node: &Node) -> String {
    let quote = super::markup::quote;
    let text = format!("#text({})", quote(&node.label));
    match &node.link {
        Some(link) if link.starts_with('#') => {
            format!("#link(label({}))[{text}]", quote(&link[1..]))
        },
        Some(link) => format!("#link({})[{text}]", quote(link)),
        None => text,
    }
}

pub(super) fn validate_external(destination: &str) -> Result<()> {
    let url = url::Url::parse(destination)?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "unresolved diagram resource: {destination}"
    );
    Ok(())
}
