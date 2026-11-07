//! Deterministic small vector panels. The renderer supplies target-specific
//! links.
use {
    super::{Diagram, Node},
    anyhow::{Result, ensure},
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
pub(crate) fn draw(diagram: &Diagram, links: bool) -> Result<String> {
    let mut nodes = BTreeMap::new(); // bounded: focus and twelve edge endpoints.
    for node in diagram.nodes() {
        nodes.insert(node.id.clone(), node);
    }
    let others: Vec<_> = nodes
        .values()
        .filter(|n| n.id != diagram.focus.id)
        .copied()
        .collect();
    let rows = others.len().div_ceil(2).max(1);
    let height = rows * 118 + 48;
    let mut positions = BTreeMap::new(); // bounded: at most thirteen nodes.
    positions.insert(
        diagram.focus.id.as_str(),
        (360usize, height / 2 - 36),
    );
    for (i, node) in others.iter().enumerate() {
        positions.insert(
            node.id.as_str(),
            (
                if i % 2 == 0 { 16 } else { 704 },
                24 + (i / 2) * 118,
            ),
        );
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 \
         {height}\" width=\"1000\" height=\"{height}\" role=\"img\" \
         aria-label=\"{} class neighborhood\"><title>{} class \
         neighborhood</title><desc>Arrows show explicitly asserted RDF \
         relationships, not required fields or property \
         ownership.</desc><rect width=\"1000\" height=\"{height}\" rx=\"8\" \
         fill=\"#f7f9fc\"/>",
        xml(&diagram.focus.label),
        xml(&diagram.focus.label)
    );
    for (index, edge) in diagram.edges.iter().enumerate() {
        let (fx, fy) = positions[edge.from.id.as_str()];
        let (tx, ty) = positions[edge.to.id.as_str()];
        let (sx, ex) = if fx < tx {
            (fx + 280, tx)
        } else {
            (fx, tx + 280)
        };
        let sy = fy + 36;
        let ey = ty + 36;
        let direction: i32 = if fx < tx { -1 } else { 1 };
        if edge.from.id == edge.to.id {
            svg.push_str(&format!(
                "<path d=\"M {} {sy} h 18 v -54 h -40 v 18\" fill=\"none\" \
                 stroke=\"#64748b\" stroke-width=\"1.5\"/>",
                fx + 280
            ));
        } else {
            let middle = (sx + ex) / 2;
            svg.push_str(&format!(
                "<path d=\"M {sx} {sy} C {middle} {sy} {middle} {ey} {ex} \
                 {ey}\" fill=\"none\" stroke=\"#64748b\" \
                 stroke-width=\"1.5\"/><path d=\"M {} {} L {ex} {ey} L {} \
                 {}\" fill=\"none\" stroke=\"#64748b\" stroke-width=\"1.5\"/>",
                ex as i32 + direction * 7,
                ey - 4,
                ex as i32 + direction * 7,
                ey + 4
            ));
        }
        let lx = (sx + ex) / 2;
        let ly = (sy + ey) / 2 - 7 - (index % 2) * 14;
        svg.push_str(&format!(
            "<text x=\"{lx}\" y=\"{ly}\" text-anchor=\"middle\" \
             font-family=\"Liberation Sans,Arial,sans-serif\" \
             font-size=\"11\" fill=\"#334155\" stroke=\"#f7f9fc\" \
             stroke-width=\"4\" paint-order=\"stroke\">{}</text>",
            xml(&edge.relation)
        ));
    }
    for node in nodes.values() {
        let (x, y) = positions[node.id.as_str()];
        draw_node(
            &mut svg,
            node,
            x,
            y,
            node.id == diagram.focus.id,
            links,
        )?;
    }
    svg.push_str("</svg>");
    Ok(svg)
}
fn draw_node(
    svg: &mut String,
    node: &Node,
    x: usize,
    y: usize,
    focus: bool,
    links: bool,
) -> Result<()> {
    if links && let Some(link) = &node.link {
        safe_link(link)?;
        svg.push_str(&format!(
            "<a href=\"{}\" aria-label=\"{}\">",
            xml(link),
            xml(&node.label)
        ));
    }
    let (fill, stroke) = if focus {
        ("#e9edff", "#4a4db3")
    } else if node.kind.contains("property") {
        ("#fff5e9", "#c36b19")
    } else {
        ("#ffffff", "#8c9bb0")
    };
    svg.push_str(&format!(
        "<g><title>{}</title><rect x=\"{x}\" y=\"{y}\" width=\"280\" \
         height=\"72\" rx=\"5\" fill=\"{fill}\" stroke=\"{stroke}\" \
         stroke-width=\"{}\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" \
         font-family=\"Liberation Sans,Arial,sans-serif\" font-size=\"11\" \
         fill=\"#64748b\">«{}»</text>",
        xml(&node.label),
        if focus { 2 } else { 1 },
        x + 140,
        y + 16,
        xml(&node.kind)
    ));
    // bounded: node labels have at most512 bytes; full text remains in
    // title/list.
    let letters: Vec<_> = node.label.chars().collect();
    let abbreviated: String = if letters.len() > 64 {
        letters[..61].iter().collect::<String>() + "…"
    } else {
        node.label.clone()
    };
    for (i, line) in abbreviated
        .chars()
        .collect::<Vec<_>>()
        .chunks(32)
        .enumerate()
    {
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" \
             font-family=\"Liberation Sans,Arial,sans-serif\" \
             font-size=\"13\" font-weight=\"600\" fill=\"#24324b\">{}</text>",
            x + 140,
            y + 36 + i * 17,
            xml(&line.iter().collect::<String>())
        ));
    }
    svg.push_str("</g>");
    if links && node.link.is_some() {
        svg.push_str("</a>");
    }
    Ok(())
}
