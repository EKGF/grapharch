//! Independent native SVG layout inspired by EKG Catalog's documented diagram
//! rules.
use {
    super::model::Tree,
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::collections::{BTreeMap, BTreeSet},
};
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub(super) fn diagram(tree: &Tree, center: &NamedNode) -> Result<String> {
    let node = &tree.nodes[center];
    let font = typst::text::Font::new(
        typst::foundations::Bytes::new(
            include_bytes!("../../render/fonts/LiberationSans-Regular.ttf")
                .as_slice(),
        ),
        0,
    )
    .ok_or_else(|| anyhow::anyhow!("invalid embedded diagram font"))?;
    let measure = |label: &str| -> Result<usize> {
        let face = font.ttf();
        let mut advance = 0u32;
        for c in label.chars() {
            let glyph = face.glyph_index(c).ok_or_else(|| {
                anyhow::anyhow!("diagram font does not contain {c:?}")
            })?;
            advance +=
                u32::from(face.glyph_hor_advance(glyph).ok_or_else(|| {
                    anyhow::anyhow!("diagram glyph has no advance")
                })?);
        }
        Ok(
            (f64::from(advance) * 13.0 / font.units_per_em()).ceil() as usize +
                32,
        )
    };
    let mut columns: [Vec<NamedNode>; 5] = std::array::from_fn(|_| Vec::new());
    if let Some(parent) = &node.parent {
        if let Some(grandparent) = &tree.nodes[parent].parent {
            columns[0].push(grandparent.clone());
        }
        columns[1].push(parent.clone());
        columns[2] = tree.children(parent, false);
    } else {
        columns[2].push(center.clone());
    }
    columns[1].extend(
        node.used_in
            .iter()
            .filter(|id| !columns[1].contains(id))
            .cloned()
            .collect::<Vec<_>>(),
    );
    columns[3] = tree.children(center, true);
    let mut grandchildren = BTreeSet::new();
    for child in &columns[3] {
        grandchildren.extend(tree.children(child, true));
    }
    columns[4] = tree.sorted(grandchildren.into_iter().collect());
    let mut seen = BTreeSet::new(); // bounded: a diagram is limited to 128 distinct nodes.
    // The center column wins when a reuse edge also makes a sibling a child.
    let mut placement = BTreeMap::new();
    for i in [2, 1, 0, 3, 4] {
        columns[i].retain(|id| seen.insert(id.clone()));
    }
    ensure!(
        seen.len() <= 128,
        "use-case diagram exceeds 128-node neighborhood budget"
    );
    let mut width = Vec::new(); // bounded: the five diagram columns.
    for nodes in &columns {
        let mut max = 0;
        for id in nodes {
            max = max.max(measure(&tree.nodes[id].label)?);
        }
        width.push(max);
    }
    let height =
        (columns.iter().map(Vec::len).max().unwrap_or(1) * 38 + 50).max(120);
    let mut x = 20usize;
    for (column, nodes) in columns.iter().enumerate() {
        if nodes.is_empty() {
            continue;
        }
        let top = (height - nodes.len() * 38) / 2;
        for (index, id) in nodes.iter().enumerate() {
            placement.insert(
                id.clone(),
                (x, top + index * 38 + 19, width[column]),
            );
        }
        x += width[column] + 55;
    }
    let total = x.max(240);
    ensure!(
        total <= 8192 && height <= 8192,
        "use-case SVG exceeds dimension budget"
    );
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{total}\" \
         height=\"{height}\" viewBox=\"0 0 {total} {height}\" \
         role=\"img\"><title>{}</title><desc>Ownership is solid; reuse is \
         dashed. Only the current use case is boxed.</desc>",
        xml(&node.label)
    );
    for (child, (cx, cy, cw)) in &placement {
        let n = &tree.nodes[child];
        for (parent, reuse) in n
            .parent
            .iter()
            .map(|p| (p, false))
            .chain(n.used_in.iter().map(|p| (p, true)))
        {
            let Some((px, py, pw)) = placement.get(parent) else {
                continue
            };
            let (sx, sy, ex, ey) = if px < cx {
                (px + pw + 3, *py, cx.saturating_sub(5), *cy)
            } else {
                (cx + cw + 3, *cy, px.saturating_sub(5), *py)
            };
            let dash = if reuse {
                " stroke-dasharray=\"5 4\""
            } else {
                ""
            };
            svg.push_str(&format!(
                "<path d=\"M {sx} {sy} H {} V {ey} H {ex}\" fill=\"none\" \
                 stroke=\"#b86512\" stroke-width=\"1.5\"{dash}/>",
                (sx + ex) / 2
            ));
        }
    }
    for (id, (x, y, w)) in &placement {
        let active = id == center;
        if active {
            svg.push_str(&format!(
                "<rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"30\" \
                 rx=\"5\" fill=\"#4051b5\" stroke=\"#d87813\" \
                 stroke-width=\"2\"/>",
                y - 15
            ));
        }
        let fill = if active { "#ffffff" } else { "#273b5c" };
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-family=\"Liberation Sans, Arial, \
             sans-serif\" font-size=\"13\" fill=\"{fill}\">{}</text>",
            x + 12,
            y + 5,
            xml(&tree.nodes[id].label)
        ));
    }
    svg.push_str("</svg>");
    crate::publication::image_assets::validate(
        svg.as_bytes(),
        "image/svg+xml",
    )?;
    Ok(svg)
}
