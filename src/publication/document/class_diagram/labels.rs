//! Keep bounded relationship captions disjoint, including dense shared ranges.
use mermaid_rs_renderer::Layout;

#[derive(Clone, Copy)]
struct Box2d {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Box2d {
    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.w &&
            self.x + self.w > other.x &&
            self.y < other.y + other.h &&
            self.y + self.h > other.y
    }
}

pub(super) fn place(layout: &mut Layout) -> String {
    let mut occupied: Vec<_> = layout
        .nodes
        .values()
        .map(|node| {
            Box2d {
                x: node.x - 4.0,
                y: node.y - 4.0,
                w: node.width + 8.0,
                h: node.height + 8.0,
            }
        })
        .collect(); // bounded: thirteen diagram nodes and six labels.
    let mut leaders = String::new();
    for edge in &mut layout.edges {
        let (Some(label), Some(original)) = (&edge.label, edge.label_anchor)
        else {
            continue
        };
        let width = label.width + 24.0;
        let height = label.height + 16.0;
        let rect = |x, y| {
            Box2d {
                x: x - width / 2.0,
                y: y - height / 2.0,
                w: width,
                h: height,
            }
        };
        let fits = |candidate: Box2d| {
            candidate.x >= 0.0 &&
                candidate.y >= 0.0 &&
                candidate.x + candidate.w <= layout.width &&
                candidate.y + candidate.h <= layout.height &&
                occupied
                    .iter()
                    .all(|existing| !candidate.intersects(*existing))
        };
        let mut selected =
            fits(rect(original.0, original.1)).then_some(original);
        if selected.is_none() {
            'segments: for segment in edge.points.windows(2) {
                for fraction in [0.5, 0.25, 0.75, 0.125, 0.875] {
                    let x =
                        segment[0].0 + (segment[1].0 - segment[0].0) * fraction;
                    let y =
                        segment[0].1 + (segment[1].1 - segment[0].1) * fraction;
                    if fits(rect(x, y)) {
                        selected = Some((x, y));
                        break 'segments;
                    }
                }
            }
        }
        let mut off_edge = false;
        if selected.is_none() {
            'rings: for radius in 1i32..=24 {
                for dx in -radius..=radius {
                    for dy in [-radius + dx.abs(), radius - dx.abs()] {
                        let point = (
                            original.0 + dx as f32 * 12.0,
                            original.1 + dy as f32 * 12.0,
                        );
                        if fits(rect(point.0, point.1)) {
                            selected = Some(point);
                            off_edge = true;
                            break 'rings;
                        }
                    }
                }
            }
        }
        let (position, needs_leader) = match selected {
            Some(position) => (position, off_edge),
            None => {
                // A caption lane gives even the densest legal panel enough
                // room. The leader retains its association with the edge.
                let position = (
                    layout.width + width / 2.0 + 16.0,
                    height / 2.0 + 16.0,
                );
                layout.width += width + 32.0;
                layout.height = layout.height.max(height + 32.0);
                (position, true)
            },
        };
        if needs_leader {
            leaders.push_str(&format!(
                "<path d=\"M {} {} L {} {}\" stroke=\"#94a3b8\" \
                 stroke-width=\"1\" stroke-dasharray=\"3 3\" fill=\"none\"/>",
                original.0, original.1, position.0, position.1
            ));
        }
        edge.label_anchor = Some(position);
        occupied.push(rect(position.0, position.1));
    }
    leaders
}
