//! Typed, presentation-independent boundaries retained in Markdown fragments.
//! Ordinary Markdown between boundaries remains visible to reference
//! collectors.
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};
pub(crate) const LANGUAGE: &str = "grapharch-content";
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Boundary {
    Alternatives,
    CardGroup,
    Figure {
        max_width_px: Option<u16>,
    },
    Caption,
    Alternative {
        title: String,
    },
    Admonition {
        category:    String,
        title:       String,
        collapsible: bool,
        expanded:    bool,
    },
    /// A numbered, captioned figure holding one or more tables. Its label
    /// is the target of [`TABLE_REFERENCE`] links.
    TableFigure {
        caption: String,
        label:   String,
    },
    /// A table whose cells carry full Markdown blocks. Rows are implied:
    /// every `columns.len()` consecutive cells form one row.
    Table {
        columns: Vec<TableColumn>,
    },
    /// One table cell; the Markdown up to its end boundary is its content.
    Cell,
    End,
}

/// A header cell and the share of the table width its column takes.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TableColumn {
    pub title:   String,
    pub percent: u8,
}

/// Links to this prefix followed by a table figure label are numbered
/// cross-references; their link text is the reference in renderers that
/// do not number tables.
pub(crate) const TABLE_REFERENCE: &str = "urn:grapharch:table:";

/// The table figure label a cross-reference destination names.
pub(crate) fn table_reference(destination: &str) -> Option<&str> {
    destination
        .strip_prefix(TABLE_REFERENCE)
        .filter(|label| crate::publication::anchor::valid(label))
}
impl Boundary {
    pub fn markdown(&self) -> Result<String> {
        Ok(format!(
            "\n```{LANGUAGE}\n{}\n```\n\n",
            serde_json::to_string(self)?
        ))
    }

    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 4096,
            "content boundary exceeds byte budget"
        );
        let value: Self = serde_json::from_str(source)?;
        match &value {
            Self::Figure { max_width_px: Some(width) } => {
                ensure!(
                    (1..=2048).contains(width),
                    "figure width must be 1–2048 pixels"
                )
            },
            Self::Alternative { title } => {
                ensure!(
                    !title.is_empty() && title.len() <= 512,
                    "invalid alternative title"
                )
            },
            Self::Admonition { category, title, .. } => {
                ensure!(
                    !category.is_empty() &&
                        category.len() <= 64 &&
                        category.chars().all(|c| {
                            c.is_ascii_alphanumeric() || c == '-' || c == '_'
                        }) &&
                        title.len() <= 512,
                    "invalid admonition metadata"
                )
            },
            Self::TableFigure { caption, label } => {
                ensure!(
                    !caption.trim().is_empty() &&
                        caption.len() <= 512 &&
                        crate::publication::anchor::valid(label),
                    "invalid table figure caption or label"
                )
            },
            Self::Table { columns } => {
                ensure!(
                    (1..=8).contains(&columns.len()) &&
                        columns.iter().all(|column| {
                            !column.title.trim().is_empty() &&
                                column.title.len() <= 128 &&
                                column.percent > 0
                        }) &&
                        columns
                            .iter()
                            .map(|column| u32::from(column.percent))
                            .sum::<u32>() ==
                            100,
                    "a table needs one to eight titled columns whose widths \
                     total 100 percent"
                )
            },
            _ => {},
        }
        Ok(value)
    }
}
#[derive(Default)]
pub(crate) struct Stack(Vec<Boundary>);
impl Stack {
    pub fn step(&mut self, value: Boundary) -> Result<Boundary> {
        if matches!(value, Boundary::End) {
            return self
                .0
                .pop()
                .ok_or_else(|| anyhow::anyhow!("unbalanced content boundary"));
        }
        ensure!(
            self.0.len() < 16,
            "content container nesting exceeds 16"
        );
        if matches!(value, Boundary::Caption) {
            ensure!(
                matches!(self.0.last(), Some(Boundary::Figure { .. })),
                "caption requires a figure"
            );
        }
        ensure!(
            matches!(value, Boundary::Cell) ==
                matches!(self.0.last(), Some(Boundary::Table { .. })),
            "table cells belong directly to a table, and tables hold only \
             cells"
        );
        if matches!(value, Boundary::Alternative { .. }) {
            ensure!(
                matches!(self.0.last(), Some(Boundary::Alternatives)),
                "alternative requires an alternatives group"
            );
        } else {
            ensure!(
                !matches!(self.0.last(), Some(Boundary::Alternatives)),
                "alternatives group requires named alternatives"
            );
        }
        self.0.push(value.clone());
        Ok(value)
    }

    /// Whether the innermost open container is a table cell.
    pub fn in_cell(&self) -> bool {
        matches!(self.0.last(), Some(Boundary::Cell))
    }

    pub fn finish(self) -> Result<()> {
        ensure!(self.0.is_empty(), "unclosed content container");
        Ok(())
    }
}

/// Retain enclosing semantic context when a publication selects a heading
/// slice.
pub(crate) struct Fragments {
    // bounded: at most 1024 boundaries in a capped Markdown body.
    markers: Vec<(usize, Boundary)>,
}
impl Fragments {
    pub fn read(body: &str) -> Result<Self> {
        use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
        let mut markers = Vec::new();
        let mut source = None;
        let mut stack = Stack::default();
        for (event, range) in
            Parser::new_ext(body, crate::publication::markdown::options())
                .into_offset_iter()
        {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                    if info.as_ref() == LANGUAGE =>
                {
                    source = Some(String::new())
                },
                Event::Text(text) if source.is_some() => {
                    source.as_mut().unwrap().push_str(&text)
                },
                Event::End(TagEnd::CodeBlock) if source.is_some() => {
                    let boundary = Boundary::parse(&source.take().unwrap())?;
                    stack.step(boundary.clone())?;
                    ensure!(
                        markers.len() < 1024,
                        "too many semantic content boundaries"
                    );
                    markers.push((range.end, boundary));
                },
                _ => {},
            }
        }
        stack.finish()?;
        Ok(Self { markers })
    }

    pub fn context(
        &self,
        start: usize,
        end: usize,
    ) -> Result<(String, String)> {
        let mut stack = Stack::default();
        let mut output = String::new();
        for (_, boundary) in self
            .markers
            .iter()
            .take_while(|(offset, _)| *offset <= start)
        {
            stack.step(boundary.clone())?;
        }
        for boundary in &stack.0 {
            output.push_str(&boundary.markdown()?);
        }
        let prefix = output;
        let mut output = String::new();
        for (_, boundary) in self
            .markers
            .iter()
            .filter(|(offset, _)| *offset > start && *offset <= end)
        {
            stack.step(boundary.clone())?;
        }
        for _ in &stack.0 {
            output.push_str(&Boundary::End.markdown()?);
        }
        Ok((prefix, output))
    }
}
