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
        "<figure class=\"owl-class-diagram\" \
         style=\"width:fit-content;max-width:100%;margin:1.25rem auto\"><div \
         tabindex=\"0\" role=\"region\" aria-label=\"Class relationship \
         diagram; scroll horizontally when needed\" \
         style=\"overflow-x:auto;background:#fff;padding:0.75rem;\
         border-radius:0.4rem\">{}</div><figcaption>Class neighborhood: {} \
         {}</figcaption></figure><details \
         class=\"owl-relationships\"><summary>Relationships as text</summary>",
        svg::draw(&projection, true)?,
        svg::xml(&diagram.focus.label),
        super::diagram_notes::reference_html()
    );
    if diagram.edges.is_empty() {
        html.push_str(
            "<p>No asserted OWL relationships in this neighborhood.</p>",
        );
    } else {
        html.push_str(
            "<div class=\"table-scroll\" role=\"region\" \
             aria-label=\"Asserted OWL relationships\" tabindex=\"0\"><table \
             class=\"owl-relationship-table\" aria-label=\"Asserted OWL \
             relationships\"><thead class=\"sr-only\"><tr><th \
             scope=\"col\">Subject</th><th scope=\"col\">Predicate</th><th \
             scope=\"col\">Object</th></tr></thead><tbody>",
        );
        for edge in &diagram.edges {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                node_html(&edge.from)?,
                svg::xml(&edge.relation),
                node_html(&edge.to)?
            ));
        }
        html.push_str("</tbody></table></div>");
    }
    for shape in &diagram.shapes {
        html.push_str(&format!(
            "<p>SHACL constraints for {} from {}</p><div \
             class=\"table-scroll\" role=\"region\" aria-label=\"SHACL \
             constraints\" tabindex=\"0\"><table \
             class=\"owl-constraint-table\"><thead><tr><th \
             scope=\"col\">Property</th><th scope=\"col\">Value type</th><th \
             scope=\"col\">Count</th></tr></thead><tbody>",
            node_html(&shape.class)?,
            node_html(&shape.source)?
        ));
        for field in &shape.fields {
            let range = field
                .range
                .as_ref()
                .map(node_html)
                .transpose()?
                .unwrap_or_else(|| "—".into());
            html.push_str(&format!(
                "<tr><td>{}</td><td>{range}</td><td>{}</td></tr>",
                node_html(&field.property)?,
                svg::xml(&field.cardinality())
            ));
        }
        html.push_str("</tbody></table></div>");
    }
    html.push_str("</details>\n");
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
    // Bound physical height as well as width. A tall neighborhood must not be
    // enlarged to a full page merely to fill the text column.
    let mut result = format!(
        "#block(breakable: false)[#layout(size => image(bytes({}), format: \
         \"svg\", width: calc.min(size.width * {}, {}pt), alt: {}))]\n",
        quote(&svg),
        if diagram.edges.is_empty() {
            "0.35"
        } else {
            "1.0"
        },
        420.0 * width / height,
        quote(&format!(
            "Class neighborhood for {}",
            diagram.focus.label
        ))
    );
    result.push_str(super::diagram_notes::reference_typst());
    // Text links remain accessible and clickable in PDF, where SVG hyperlink
    // annotations are not retained by the image decoder.
    if !diagram.edges.is_empty() {
        result.push_str(
            "#text(size: 8pt)[#ga-table(columns: (1fr, auto, 1fr), \
             has-header: false, ",
        );
        for edge in &diagram.edges {
            result.push_str(&format!(
                "[{}], [#text({})], [{}], ",
                node_typst(&edge.from),
                quote(&edge.relation),
                node_typst(&edge.to)
            ));
        }
        result.push_str(")]\n");
    }
    for shape in &diagram.shapes {
        result.push_str(&format!(
            "#par[SHACL constraints for {} from {}]\n",
            node_typst(&shape.class),
            node_typst(&shape.source)
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::publication::document::class_diagram::{Diagram, Edge},
    };
    fn retained(link: &str) -> String {
        let node = Node {
            id:    "urn:test:class".into(),
            label: "Class".into(),
            kind:  "Class".into(),
            link:  Some(link.into()),
        };
        serde_json::to_string(&Projection {
            diagram: Diagram {
                focus:   node.clone(),
                shapes:  Vec::new(),
                limited: false,
                edges:   vec![Edge {
                    predicate: None,
                    from:      node.clone(),
                    relation:  "rdfs:subClassOf".into(),
                    to:        node,
                }],
            },
            source:  "@startuml\n@enduml\n".into(),
            engine:  "PlantUML version fixture".into(),
            svg:     "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" \
                      height=\"800\" viewBox=\"0 0 400 800\"><a \
                      href=\"./grapharch-node/n0\"><rect width=\"400\" \
                      height=\"800\"/></a></svg>"
                .into(),
        })
        .unwrap()
    }
    #[test]
    fn textual_relationships_are_a_full_width_accessible_table() {
        let output = html(&retained("#class")).unwrap();
        let parsed = scraper::Html::parse_fragment(&output);
        let details = parsed
            .select(
                &scraper::Selector::parse("details.owl-relationships").unwrap(),
            )
            .next()
            .expect("relationship disclosure");
        assert!(!details.ancestors().any(|node| {
            scraper::ElementRef::wrap(node)
                .is_some_and(|element| element.value().name() == "figure")
        }));
        let table = details
            .select(&scraper::Selector::parse("table").unwrap())
            .next()
            .expect("three-column relationship table");
        assert_eq!(
            table.value().attr("aria-label"),
            Some("Asserted OWL relationships")
        );
        assert_eq!(
            table
                .select(
                    &scraper::Selector::parse("thead.sr-only th[scope=col]")
                        .unwrap()
                )
                .count(),
            3
        );
        assert_eq!(
            table
                .select(&scraper::Selector::parse("tbody tr td").unwrap())
                .count(),
            3
        );
        assert!(table.text().collect::<String>().contains("rdfs:subClassOf"));
        assert_eq!(
            details
                .select(&scraper::Selector::parse("ul,li").unwrap())
                .count(),
            0
        );
        assert!(!output.contains(" — ") && !output.contains(" → "));
    }
    #[test]
    fn tall_pdf_drawings_have_a_physical_height_budget_and_text_links() {
        let output = typst(&retained("#class")).unwrap();
        assert!(output.contains("calc.min(size.width * 1.0, 210pt)"));
        assert!(output.contains("#link(label(\"class\"))"));
        assert!(!output.contains("grapharch-node"));
    }
    #[test]
    fn phone_diagrams_keep_readable_intrinsic_width_inside_a_scroll_region() {
        let output = html(&retained("#class")).unwrap();
        assert!(output.contains("min-width:340px"));
        assert!(output.contains("overflow-x:auto"));
        assert!(output.contains("tabindex=\"0\""));
    }
    #[test]
    fn retained_svg_links_reject_executable_destinations() {
        assert!(html(&retained("javascript:alert(1)")).is_err());
        assert!(html(&retained("//evil.example")).is_err());
        assert!(
            html(&retained("#class"))
                .unwrap()
                .contains("href=\"#class\"")
        );
    }
}
