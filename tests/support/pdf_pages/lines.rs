//! Glyphs grouped into the text lines of a page.
use {
    super::{font::FontFamily, glyphs::PlacedGlyph},
    std::collections::{BTreeMap, BTreeSet},
};

/// One baseline of text, left to right.
#[derive(Clone, Debug)]
pub struct TextLine {
    /// Baseline distance from the top of the page, in points.
    pub y:     f64,
    /// Left edge of the first glyph, in points.
    pub x:     f64,
    /// Right edge of the last glyph, in points.
    pub right: f64,
    /// The largest glyph size on the line, in points.
    pub size:  f64,
    /// The font of the line's first glyph.
    pub font:  Option<FontFamily>,
    pub text:  String,
    /// The runs of the line separated by gaps wider than its size, such as
    /// the cells of a table row.
    pub runs:  Vec<TextRun>, // bounded: the runs of one line.
}

/// Text of a line between two wide gaps.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub x:    f64,
    pub text: String,
    pub font: Option<FontFamily>,
}

/// The size and text lines of one page, top to bottom.
#[derive(Clone, Debug)]
pub struct PageText {
    pub width:  f64,
    pub height: f64,
    pub lines:  Vec<TextLine>, // bounded: the lines of one page.
    /// Every font a letter or digit on the page is set in; symbols may come
    /// from fallback fonts.
    pub fonts:  BTreeSet<FontFamily>,
}

impl PageText {
    pub(super) fn new(
        width: f64,
        height: f64,
        glyphs: Vec<PlacedGlyph>,
    ) -> Self {
        // Baselines within a fifth of a point are one line.
        let mut rows = BTreeMap::<i64, Vec<PlacedGlyph>>::new();
        let fonts = glyphs
            .iter()
            .filter(|glyph| {
                glyph.text.chars().all(|c| c.is_ascii_alphanumeric())
            })
            .filter_map(|glyph| glyph.font.clone())
            .collect();
        for glyph in glyphs {
            rows.entry((glyph.y * 5.0).round() as i64)
                .or_default()
                .push(glyph);
        }
        let lines = rows
            .into_values()
            .filter_map(|mut row| {
                row.sort_by(|left, right| left.x.total_cmp(&right.x));
                TextLine::join(&row)
            })
            .collect();
        Self { width, height, lines, fonts }
    }

    /// Lines in the bottom `margin` points of the page.
    pub fn footer(&self, margin: f64) -> impl Iterator<Item = &TextLine> {
        self.lines
            .iter()
            .filter(move |line| line.y > self.height - margin)
    }

    /// Lines between the top and bottom `margin` points of the page.
    pub fn body(&self, margin: f64) -> impl Iterator<Item = &TextLine> {
        self.lines.iter().filter(move |line| {
            line.y > margin && line.y < self.height - margin
        })
    }
}

impl TextLine {
    fn join(row: &[PlacedGlyph]) -> Option<Self> {
        let first = row.first()?;
        let mut text = String::new();
        let mut runs = Vec::<TextRun>::new();
        let mut end = first.x;
        let mut size: f64 = 0.0;
        for glyph in row {
            let gap = glyph.x - end;
            // A gap wider than a fifth of the glyph size is a word space;
            // one wider than half the glyph size separates runs, such as the
            // cells of a table row, whose padding is wider than a word space.
            if !text.is_empty() &&
                gap > glyph.size * 0.2 &&
                !text.ends_with(' ')
            {
                text.push(' ');
            }
            if runs.is_empty() || gap > glyph.size * 0.5 {
                runs.push(TextRun {
                    x:    glyph.x,
                    text: String::new(),
                    font: glyph.font.clone(),
                });
            } else if gap > glyph.size * 0.2 &&
                !runs.last().is_some_and(|run| run.text.ends_with(' '))
            {
                runs.last_mut().expect("current run").text.push(' ');
            }
            // Typst joins a supplement and its number with a no-break space.
            let glyph_text = glyph.text.replace('\u{a0}', " ");
            text.push_str(&glyph_text);
            runs.last_mut()
                .expect("current run")
                .text
                .push_str(&glyph_text);
            end = end.max(glyph.x + glyph.advance);
            size = size.max(glyph.size);
        }
        let text = text.trim().to_owned();
        runs.retain_mut(|run| {
            run.text = run.text.trim().to_owned();
            !run.text.is_empty()
        });
        (!text.is_empty()).then(|| {
            Self {
                y: first.y,
                x: first.x,
                right: end,
                size,
                font: first.font.clone(),
                text,
                runs,
            }
        })
    }
}
