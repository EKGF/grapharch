//! Laid-out geometry of the authority status ribbon. The specification
//! website anchors the rotated label to the ribbon's top end with a fixed
//! padding (`.authority-banner`: `writing-mode: vertical-rl`, rotated 180deg,
//! `justify-content: flex-end`, `padding: 1.5rem`); the PDF must match.

use {
    super::*,
    typst::{
        layout::{Frame, FrameItem, Point, Transform},
        visualize::Geometry,
    },
};

const RIBBON_WIDTH_MM: f64 = 8.0;
const LABEL_PADDING_MM: f64 = 6.0;
const TOLERANCE_MM: f64 = 0.25;

/// Bounding box of a laid-out element in page millimetres.
#[derive(Clone, Copy, Debug)]
struct Extent {
    top:    f64,
    bottom: f64,
    left:   f64,
    right:  f64,
}

impl Extent {
    fn of(corners: [Point; 4]) -> Self {
        let xs = corners.map(|corner| corner.x.to_mm());
        let ys = corners.map(|corner| corner.y.to_mm());
        Self {
            top:    ys.into_iter().fold(f64::INFINITY, f64::min),
            bottom: ys.into_iter().fold(f64::NEG_INFINITY, f64::max),
            left:   xs.into_iter().fold(f64::INFINITY, f64::min),
            right:  xs.into_iter().fold(f64::NEG_INFINITY, f64::max),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            top:    self.top.min(other.top),
            bottom: self.bottom.max(other.bottom),
            left:   self.left.min(other.left),
            right:  self.right.max(other.right),
        }
    }

    fn within_ribbon(self) -> bool {
        self.left >= -TOLERANCE_MM &&
            self.right <= RIBBON_WIDTH_MM + TOLERANCE_MM
    }
}

/// The ribbon bar and its label, as found on one laid-out page.
#[derive(Default)]
struct Ribbon {
    bar:   Option<Extent>,
    label: Option<Extent>,
}

impl Ribbon {
    fn on(frame: &Frame) -> Self {
        let mut ribbon = Self::default();
        ribbon.visit(frame, Transform::identity());
        ribbon
    }

    fn visit(&mut self, frame: &Frame, transform: Transform) {
        for (position, item) in frame.items() {
            let at = transform
                .pre_concat(Transform::translate(position.x, position.y));
            match item {
                FrameItem::Group(group) => {
                    self.visit(&group.frame, at.pre_concat(group.transform))
                },
                FrameItem::Shape(shape, _) => {
                    if let Geometry::Rect(size) = shape.geometry {
                        let extent = Extent::of(
                            [
                                Point::zero(),
                                Point::with_x(size.x),
                                Point::with_y(size.y),
                                Point::new(size.x, size.y),
                            ]
                            .map(|corner| corner.transform(at)),
                        );
                        if (size.x.to_mm() - RIBBON_WIDTH_MM).abs() <
                            TOLERANCE_MM &&
                            extent.left.abs() < TOLERANCE_MM
                        {
                            self.bar = Some(extent);
                        }
                    }
                },
                FrameItem::Text(text) => {
                    // The cap-height-to-baseline box is what the label's
                    // layout centres and what the eye reads as the label.
                    let cap = text.font.metrics().cap_height.at(text.size);
                    let width = text.width();
                    let extent = Extent::of(
                        [
                            Point::with_y(-cap),
                            Point::new(width, -cap),
                            Point::zero(),
                            Point::with_x(width),
                        ]
                        .map(|corner| corner.transform(at)),
                    );
                    if extent.within_ribbon() {
                        self.label = Some(
                            self.label
                                .map_or(extent, |label| label.union(extent)),
                        );
                    }
                },
                _ => {},
            }
        }
    }
}

async fn layout(style: &str, label: &str) -> Result<PagedDocument> {
    let root = tempfile::tempdir()?;
    let source = format!(
        "#let ga-style = \"{style}\"\n#let ga-accent = rgb(\"#2b5f9f\")\n#let \
         ga-title = \"Ribbon geometry\"\n#let ga-publisher = \"Example \
         Authority\"\n#let ga-ribbon = \
         \"{label}\"\n{}\n#ga-cover(ga-publisher, ga-title, [], \
         [])\n#pagebreak()\n= Chapter\nBody.\n",
        include_str!("../typst/theme.typ")
    );
    std::fs::write(root.path().join("main.typ"), source)?;
    let assets = Assets::load().await?;
    let path = root.path().to_owned();
    tokio::task::spawn_blocking(move || {
        let world = DocumentWorld {
            root: path,
            assets,
            main: FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new("main.typ")?,
            )),
        };
        typst::compile::<PagedDocument>(&world)
            .output
            .map_err(|diagnostics| {
                anyhow::anyhow!("Typst compilation failed: {diagnostics:?}")
            })
    })
    .await?
}

#[tokio::test]
async fn ribbon_label_is_anchored_to_the_top_end_like_the_website() -> Result<()>
{
    let labels = [
        "EKGF Editor's Draft",
        "W3C Recommendation",
        "International Council for Open Standards and Interoperability Public \
         review documentation preview for implementing organizations",
    ];
    for style in ["specification", "reader", "overview"] {
        for label in labels {
            let document = layout(style, label).await?;
            ensure!(
                document.pages().len() >= 2,
                "cover and body pages expected"
            );
            for (index, page) in document.pages().iter().enumerate() {
                let ribbon = Ribbon::on(&page.frame);
                let context = format!("{style} page {} {label:?}", index + 1);
                let bar = ribbon.bar.ok_or_else(|| {
                    anyhow::anyhow!("no ribbon bar: {context}")
                })?;
                let text = ribbon.label.ok_or_else(|| {
                    anyhow::anyhow!("no ribbon label: {context}")
                })?;
                let leading = text.top - bar.top;
                let trailing = bar.bottom - text.bottom;
                ensure!(
                    (leading - LABEL_PADDING_MM).abs() < TOLERANCE_MM,
                    "label must end {LABEL_PADDING_MM} mm below the ribbon \
                     top, found {leading:.2} mm: {context}"
                );
                ensure!(
                    trailing >= LABEL_PADDING_MM - TOLERANCE_MM,
                    "label must keep the bottom padding, found {trailing:.2} \
                     mm: {context}"
                );
                let centre = (text.left + text.right) / 2.0;
                ensure!(
                    (centre - RIBBON_WIDTH_MM / 2.0).abs() < TOLERANCE_MM,
                    "label must be centred across the ribbon, found \
                     {centre:.2} mm: {context}"
                );
            }
        }
    }
    Ok(())
}
