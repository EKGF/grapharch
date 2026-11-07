//! Deterministic small vector panels. The renderer supplies target-specific
//! links.
use {
    super::Diagram,
    anyhow::{Result, ensure},
    mermaid_rs_renderer::{
        Direction,
        EdgeStyle,
        Graph,
        LayoutConfig,
        NodeShape,
        Theme,
        compute_layout,
        render_svg,
    },
    sha2::{Digest, Sha256},
    std::collections::BTreeMap,
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
    ensure!(
        !value.chars().any(char::is_control) && !value.contains('\\'),
        "invalid diagram link"
    );
    ensure!(
        value.starts_with('#') ||
            value.starts_with("./") ||
            value.starts_with("../") ||
            value.starts_with("https://") ||
            value.starts_with("http://"),
        "unsupported class diagram link {value}"
    );
    Ok(())
}
fn graph(diagram: &Diagram, links: bool) -> Result<Graph> {
    let mut graph = Graph::new();
    graph.kind = mermaid_rs_renderer::DiagramKind::Class;
    graph.direction = Direction::LeftRight;
    let mut nodes = BTreeMap::new(); // bounded: focus and twelve edge endpoints.
    for node in diagram.nodes() {
        nodes.insert(node.id.clone(), node);
    }
    let mut identifiers = BTreeMap::new(); // bounded: thirteen diagram nodes.
    for (index, node) in nodes.values().enumerate() {
        let id = format!("n{index}");
        identifiers.insert(node.id.as_str(), id.clone());
        graph.node_order.insert(id.clone(), index);
        graph.nodes.insert(id.clone(), mermaid_rs_renderer::Node {
            id:    id.clone(),
            label: format!("«{}»\n{}", node.kind, node.label),
            shape: NodeShape::Rectangle,
            value: None,
            icon:  None,
        });
        let focus = node.id == diagram.focus.id;
        let (fill, stroke) = if focus {
            ("#e9edff", "#4a4db3")
        } else if node.kind.contains("property") {
            ("#fff5e9", "#c36b19")
        } else {
            ("#ffffff", "#8c9bb0")
        };
        graph.node_styles.insert(
            id.clone(),
            mermaid_rs_renderer::ir::NodeStyle {
                fill: Some(fill.into()),
                stroke: Some(stroke.into()),
                text_color: Some("#24324b".into()),
                stroke_width: Some(if focus { 2.0 } else { 1.0 }),
                ..Default::default()
            },
        );
        if links && let Some(link) = &node.link {
            safe_link(link)?;
            graph.node_links.insert(id, mermaid_rs_renderer::NodeLink {
                url:    link.clone(),
                title:  Some(node.label.clone()),
                target: None,
            });
        }
    }
    for edge in &diagram.edges {
        graph.edges.push(mermaid_rs_renderer::Edge {
            from:             identifiers[edge.from.id.as_str()].clone(),
            to:               identifiers[edge.to.id.as_str()].clone(),
            label:            Some(edge.relation.clone()),
            start_label:      None,
            end_label:        None,
            directed:         true,
            arrow_start:      false,
            arrow_end:        true,
            arrow_start_kind: None,
            arrow_end_kind:   None,
            start_decoration: None,
            end_decoration:   None,
            style:            EdgeStyle::Solid,
        });
    }
    Ok(graph)
}
fn configuration() -> LayoutConfig {
    LayoutConfig {
        node_spacing: 48.0,
        rank_spacing: 96.0,
        max_label_width_chars: 34,
        preferred_aspect_ratio: Some(1.8),
        ..Default::default()
    }
}
pub(crate) fn draw(diagram: &Diagram, links: bool) -> Result<String> {
    let graph = graph(diagram, links)?;
    let config = configuration();
    let theme = Theme::modern();
    let mut layout = compute_layout(&graph, &theme, &config);
    let leaders = super::labels::place(&mut layout);
    let svg = render_svg(&layout, &theme, &config);
    let start = svg
        .find('>')
        .ok_or_else(|| anyhow::anyhow!("invalid diagram SVG"))?;
    let mut result = format!(
        "{}<title>{} class neighborhood</title><desc>Arrows show explicit OWL \
         and RDFS relationships; properties are independent \
         resources.</desc>{}",
        &svg[..=start],
        xml(&diagram.focus.label),
        &svg[start + 1..]
    );
    result = result.replace("</svg>", &format!("{leaders}</svg>"));
    result = result.replacen(
        "<svg ",
        &format!(
            "<svg role=\"img\" aria-label=\"{} class neighborhood\" ",
            xml(&diagram.focus.label)
        ),
        1,
    );
    let namespace = format!(
        "owl-{}-",
        hex::encode(Sha256::digest(serde_json::to_vec(diagram)?))
    );
    let document = roxmltree::Document::parse(&result)?;
    let ids: Vec<String> = document
        .descendants()
        .filter_map(|node| node.attribute("id").map(str::to_owned))
        .collect(); // bounded: native renderer output for at most six edges.
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
    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::publication::document::class_diagram::{Edge, Node},
    };
    #[test]
    fn property_relationship_labels_do_not_overlap_class_boxes() {
        let node = |id: &str, kind: &str| {
            Node {
                id:    id.into(),
                label: id.into(),
                kind:  kind.into(),
                link:  None,
            }
        };
        let focus = node("Core", "Class");
        let p = node("property", "Object property");
        let target = node("Target", "Class");
        let model = Diagram {
            focus: focus.clone(),
            edges: vec![
                Edge {
                    from:     p.clone(),
                    relation: "rdfs:domain".into(),
                    to:       focus,
                },
                Edge {
                    from:     p,
                    relation: "rdfs:range".into(),
                    to:       target,
                },
            ],
        };
        let graph = graph(&model, false).unwrap();
        let mut layout =
            compute_layout(&graph, &Theme::modern(), &configuration());
        super::super::labels::place(&mut layout);
        for edge in &layout.edges {
            let (x, y) =
                edge.label_anchor.expect("positioned relationship label");
            for node in layout.nodes.values() {
                assert!(
                    !(x >= node.x &&
                        x <= node.x + node.width &&
                        y >= node.y &&
                        y <= node.y + node.height),
                    "relationship label overlaps {}",
                    node.id
                );
            }
        }
    }
    #[test]
    fn shared_domain_and_range_labels_do_not_overlap_each_other() {
        let node = |id: &str, kind: &str| {
            Node {
                id:    id.into(),
                label: format!("ekg-artdep:{id}"),
                kind:  kind.into(),
                link:  None,
            }
        };
        let focus = node("DependencyEvidence", "Class");
        let other = node("Dependency", "Class");
        let resource = node("Resource", "Resource");
        let p = node("dependencyEvidence", "Object property");
        let q = node("evidenceGraph", "Object property");
        let r = node("evidenceObject", "Object property");
        let edge = |from: Node, relation: &str, to: Node| {
            Edge { from, relation: relation.into(), to }
        };
        let diagram = Diagram {
            focus: focus.clone(),
            edges: vec![
                edge(p.clone(), "rdfs:domain", other),
                edge(p, "rdfs:range", focus.clone()),
                edge(q.clone(), "rdfs:domain", focus.clone()),
                edge(q, "rdfs:range", resource.clone()),
                edge(r.clone(), "rdfs:domain", focus),
                edge(r, "rdfs:range", resource),
            ],
        };
        let graph = graph(&diagram, false).unwrap();
        let mut layout =
            compute_layout(&graph, &Theme::modern(), &configuration());
        super::super::labels::place(&mut layout);
        for (i, a) in layout.edges.iter().enumerate() {
            let (ax, ay) = a.label_anchor.unwrap();
            let al = a.label.as_ref().unwrap();
            for b in &layout.edges[i + 1..] {
                let (bx, by) = b.label_anchor.unwrap();
                let bl = b.label.as_ref().unwrap();
                assert!(
                    (ax - bx).abs() >= (al.width + bl.width) / 2.0 + 8.0 ||
                        (ay - by).abs() >=
                            (al.height + bl.height) / 2.0 + 8.0,
                    "overlapping relationship captions"
                );
            }
        }
    }
}
