//! PDF hit regions from bounded retained PlantUML geometry, never source files.
use {
    anyhow::{Result, ensure},
    roxmltree::Node,
};

#[derive(Clone, Copy, Debug)]
struct Bounds {
    x:      f64,
    y:      f64,
    width:  f64,
    height: f64,
}

impl Bounds {
    fn read(node: Node<'_, '_>) -> Result<Self> {
        ensure!(
            !node.ancestors().any(|n| n.attribute("transform").is_some()),
            "transformed diagram links are unsupported"
        );
        let number = |name: &str, default: Option<f64>| -> Result<f64> {
            let value = match node.attribute(name) {
                Some(value) => {
                    value.strip_suffix("px").unwrap_or(value).parse()?
                },
                None => {
                    default.ok_or_else(|| {
                        anyhow::anyhow!("diagram link geometry requires {name}")
                    })?
                },
            };
            ensure!(
                f64::is_finite(value),
                "nonfinite diagram link geometry"
            );
            Ok(value)
        };
        let x = number("x", Some(0.0))?;
        let y = number("y", Some(0.0))?;
        let bounds = match node.tag_name().name() {
            "rect" => {
                Self {
                    x,
                    y,
                    width: number("width", None)?,
                    height: number("height", None)?,
                }
            },
            "text" => {
                ensure!(
                    node.children().all(|n| n.is_text()),
                    "unsupported nested SVG link text geometry"
                );
                ensure!(
                    [
                        "dx",
                        "dy",
                        "rotate",
                        "dominant-baseline",
                        "alignment-baseline"
                    ]
                    .iter()
                    .all(|name| node.attribute(*name).is_none()) &&
                        node.attribute("text-anchor")
                            .is_none_or(|value| value == "start"),
                    "unsupported positioned SVG link text"
                );
                let font = number("font-size", None)?;
                // PlantUML supplies a baseline and explicit textLength. The
                // clickable line box includes the font's descender space.
                Self {
                    x,
                    y: y - font,
                    width: number("textLength", None)?,
                    height: font * 1.25,
                }
            },
            _ => {
                anyhow::bail!(
                    "unsupported diagram link geometry {}",
                    node.tag_name().name()
                )
            },
        };
        ensure!(
            bounds.width > 0.0 && bounds.height > 0.0,
            "empty diagram link geometry"
        );
        Ok(bounds)
    }

    fn subtract(self, other: Self, output: &mut Vec<Self>) {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        if left >= right || top >= bottom {
            output.push(self);
            return;
        }
        for (x, y, width, height) in [
            (self.x, self.y, self.width, top - self.y),
            (
                self.x,
                bottom,
                self.width,
                self.y + self.height - bottom,
            ),
            (self.x, top, left - self.x, bottom - top),
            (
                right,
                top,
                self.x + self.width - right,
                bottom - top,
            ),
        ] {
            if width > 0.0 && height > 0.0 {
                output.push(Self { x, y, width, height });
            }
        }
    }
}

fn geometry(anchor: Node<'_, '_>) -> Result<Vec<Bounds>> {
    // bounded: retained SVG capped at 256 KiB; no graph/corpus traversal.
    let elements: Vec<_> = anchor
        .descendants()
        .filter(|node| {
            node.is_element() &&
                *node != anchor &&
                node.ancestors().skip(1).find(|n| n.has_tag_name("a")) ==
                    Some(anchor)
        })
        .collect();
    let rectangles: Vec<_> = elements
        .iter()
        .copied()
        .filter(|n| n.has_tag_name("rect"))
        .collect();
    let shapes = if rectangles.is_empty() &&
        elements.iter().any(|n| n.has_tag_name("path"))
    {
        // PlantUML links the association path but emits its label as sibling
        // text inside the same typed link group. Link that precise label box,
        // never the large bounding rectangle around the curved connector.
        let group = anchor.parent_element().ok_or_else(|| {
            anyhow::anyhow!("diagram association has no group")
        })?;
        ensure!(
            group.has_tag_name("g") &&
                group.attribute("class") == Some("link") &&
                group.children().filter(|n| n.has_tag_name("a")).count() ==
                    1,
            "ambiguous diagram association link group"
        );
        group
            .children()
            .filter(|n| n.has_tag_name("text"))
            .collect()
    } else if rectangles.is_empty() {
        ensure!(
            elements.iter().all(|n| {
                n.has_tag_name("g") ||
                    n.has_tag_name("text") ||
                    n.has_tag_name("title")
            }),
            "unsupported diagram link shape"
        );
        elements
            .into_iter()
            .filter(|n| n.has_tag_name("text"))
            .collect()
    } else {
        rectangles
    };
    ensure!(
        !shapes.is_empty(),
        "diagram link has no supported hit region"
    );
    shapes.into_iter().map(Bounds::read).collect()
}

pub(super) fn typst(svg: &str) -> Result<String> {
    let document = roxmltree::Document::parse(svg)?;
    let mut output = String::new(); // bounded: maximum 1024 link pieces per diagram.
    let mut count = 0;
    for anchor in document.descendants().filter(|n| n.has_tag_name("a")) {
        let Some(destination) = anchor.attribute("href").or_else(|| {
            anchor.attribute(("http://www.w3.org/1999/xlink", "href"))
        }) else {
            continue
        };
        super::svg::safe_link(destination)?;
        let destination = if let Some(label) = destination.strip_prefix('#') {
            ensure!(!label.is_empty(), "empty PDF diagram anchor");
            format!("label({})", super::super::markup::quote(label))
        } else {
            super::validate_external(destination)?;
            super::super::markup::quote(destination)
        };
        let mut regions = geometry(anchor)?;
        // An attribute link inside a class owns its rectangle exclusively.
        // Split the surrounding class hit area instead of relying on viewer
        // ordering of overlapping PDF annotations.
        for nested in anchor.descendants().filter(|n| {
            n.has_tag_name("a") &&
                *n != anchor &&
                (n.attribute("href").is_some() ||
                    n.attribute(("http://www.w3.org/1999/xlink", "href"))
                        .is_some())
        }) {
            for exclusion in geometry(nested)? {
                let mut next = Vec::new(); // bounded by the explicit link-piece budget below.
                for region in regions {
                    region.subtract(exclusion, &mut next);
                }
                ensure!(
                    next.len() <= 1024,
                    "diagram link geometry exceeds region budget"
                );
                regions = next;
            }
        }
        for region in regions {
            count += 1;
            ensure!(
                count <= 1024,
                "diagram link geometry exceeds region budget"
            );
            output.push_str(&format!(
                "#place(top + left, dx: ga-scale * {}, dy: ga-scale * \
                 {})[#link({destination})[#box(width: ga-scale * {}, height: \
                 ga-scale * {})[]]]\n",
                region.x, region.y, region.width, region.height
            ));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_member_targets_own_their_text_region() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><a href="#class"><rect x="10" y="20" width="100" height="80"/><a href="https://example.org/member"><text x="20" y="60" font-size="12" textLength="70">member</text></a></a></svg>"##;
        let output = typst(svg).unwrap();
        assert_eq!(output.matches("#place(").count(), 5);
        assert_eq!(
            output.matches("#link(label(\"class\"))").count(),
            4
        );
        assert!(output.contains("dx: ga-scale * 20, dy: ga-scale * 48)[#link(\"https://example.org/member\")[#box(width: ga-scale * 70, height: ga-scale * 15)"));
        assert!(
            !output.contains("width: ga-scale * 100, height: ga-scale * 80")
        );
    }

    #[test]
    fn unsupported_or_unresolved_hit_geometry_fails_instead_of_mislinking() {
        for shape in [
            r#"<g transform="translate(10)"><rect width="30" height="20"/></g>"#,
            r#"<text x="0" y="14" font-size="14">unsized</text>"#,
            r#"<circle cx="10" cy="10" r="10"/>"#,
        ] {
            assert!(
                typst(&format!(
                    r##"<svg><a href="#node">{shape}</a></svg>"##
                ))
                .is_err()
            );
        }
        assert!(typst(r#"<svg><a href="../other.html"><rect width="30" height="20"/></a></svg>"#).is_err());
    }
}

#[cfg(test)]
mod associations {
    #[test]
    fn linked_curves_use_their_exact_sibling_label_not_a_large_path_rectangle()
    {
        let output = super::typst(r##"<svg><g class="link"><a href="#property"><polygon points="0,0 2,3 4,0"/><path d="M0,0 C0,200 300,200 300,0"/></a><text x="40" y="80" textLength="90" font-size="14">relationship</text></g></svg>"##).unwrap();
        assert_eq!(output.matches("#place(").count(), 1);
        assert!(output.contains("dx: ga-scale * 40, dy: ga-scale * 66"));
        assert!(
            output.contains("width: ga-scale * 90, height: ga-scale * 17.5")
        );
    }
}
