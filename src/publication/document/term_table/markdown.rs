//! Markdown of captioned reference tables, as semantic content boundaries
//! that every renderer projects.
use {
    crate::publication::content_blocks::{
        Boundary,
        TABLE_REFERENCE,
        TableColumn,
    },
    anyhow::{Result, ensure},
    sha2::{Digest, Sha256},
};

/// The largest table fragment body.
const BYTES: usize = 1024 * 1024;

/// The stable label of one table figure of one ontology.
#[derive(Clone)]
pub(super) struct TableLabel(String);

impl TableLabel {
    pub fn new(ontology: &oxrdf::NamedNode, part: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(ontology.as_str().as_bytes());
        digest.update([0]);
        digest.update(part.as_bytes());
        Self(format!(
            "ga-table-{}",
            hex::encode(&digest.finalize()[..12])
        ))
    }

    /// A numbered cross-reference whose link text is `text`.
    pub fn reference(&self, text: &str) -> String {
        format!(
            "[{}]({TABLE_REFERENCE}{})",
            super::super::annotated_term::escape(text),
            self.0
        )
    }
}

/// A Markdown body holding captioned tables.
pub(super) struct TableMarkdown {
    body: String,
    open: usize,
}

impl TableMarkdown {
    pub fn new() -> Self { Self { body: String::new(), open: 0 } }

    /// Ordinary Markdown blocks.
    pub fn paragraph(&mut self, markdown: &str) {
        self.body.push_str(markdown);
        self.body.push_str("\n\n");
    }

    fn boundary(&mut self, boundary: Boundary) -> Result<()> {
        if matches!(boundary, Boundary::End) {
            ensure!(self.open > 0, "unbalanced table boundary");
            self.open -= 1;
        } else {
            self.open += 1;
        }
        self.body.push_str(&boundary.markdown()?);
        ensure!(
            self.body.len() <= BYTES,
            "reference table exceeds 1 MiB"
        );
        Ok(())
    }

    pub fn figure(
        &mut self,
        caption: String,
        label: &TableLabel,
    ) -> Result<()> {
        self.boundary(Boundary::TableFigure { caption, label: label.0.clone() })
    }

    pub fn table(&mut self, columns: &[(&str, u8)]) -> Result<()> {
        self.boundary(Boundary::Table {
            columns: columns
                .iter()
                .map(|(title, percent)| {
                    TableColumn { title: (*title).into(), percent: *percent }
                })
                .collect(),
        })
    }

    /// One cell holding Markdown blocks; empty cells stay empty.
    pub fn cell(&mut self, markdown: &str) -> Result<()> {
        self.boundary(Boundary::Cell)?;
        if !markdown.trim().is_empty() {
            self.paragraph(markdown);
        }
        self.end()
    }

    pub fn end(&mut self) -> Result<()> { self.boundary(Boundary::End) }

    pub fn finish(self) -> Result<String> {
        ensure!(self.open == 0, "unclosed table boundary");
        Ok(self.body)
    }
}
