//! Authored caption and lead-in sentences with named placeholders.
use {
    super::super::annotated_term::escape,
    anyhow::{Result, ensure},
};

/// An authored plain-text pattern. `{title}` stands for the ontology chapter
/// title; a lead-in also needs `{table}`, the numbered table reference.
#[derive(Clone, Debug)]
pub(super) struct TextPattern(String);

impl TextPattern {
    const TABLE: &'static str = "{table}";
    const TITLE: &'static str = "{title}";

    /// A caption pattern: one `{title}`, no `{table}`.
    pub fn caption(text: &str) -> Result<Self> {
        Self::validate(text, false)?;
        Ok(Self(text.into()))
    }

    /// A lead-in pattern: at most one `{title}` and exactly one `{table}`.
    pub fn lead_in(text: &str) -> Result<Self> {
        Self::validate(text, true)?;
        Ok(Self(text.into()))
    }

    fn validate(text: &str, lead_in: bool) -> Result<()> {
        ensure!(
            !text.trim().is_empty() &&
                text.len() <= 512 &&
                !text.contains(['\n', '\r']),
            "table caption and lead-in patterns must be one nonempty line of \
             at most 512 bytes"
        );
        let titles = text.matches(Self::TITLE).count();
        let tables = text.matches(Self::TABLE).count();
        let rest = text.replace(Self::TITLE, "").replace(Self::TABLE, "");
        ensure!(
            !rest.contains(['{', '}']),
            "unknown placeholder in pattern {text:?}; use {{title}} or \
             {{table}}"
        );
        if lead_in {
            ensure!(
                titles <= 1 && tables == 1,
                "a lead-in pattern needs exactly one {{table}} and at most \
                 one {{title}}: {text:?}"
            );
        } else {
            ensure!(
                titles == 1 && tables == 0,
                "a caption pattern needs exactly one {{title}}: {text:?}"
            );
        }
        Ok(())
    }

    /// The plain text, with `title` in place of `{title}`.
    pub fn text(&self, title: &str) -> String {
        self.0.replace(Self::TITLE, title)
    }

    /// Inert Markdown, with the escaped `title` and `table` Markdown in place
    /// of their placeholders.
    pub fn markdown(&self, title: &str, table: &str) -> String {
        let mut output = String::new();
        for (index, part) in self.0.split(Self::TABLE).enumerate() {
            if index > 0 {
                output.push_str(table);
            }
            output.push_str(
                &part
                    .split(Self::TITLE)
                    .map(|segment| {
                        // Keep the spaces that separate a placeholder from
                        // its neighbours; escape() trims them.
                        let lead = segment.starts_with(' ');
                        let trail = segment.ends_with(' ') && segment.len() > 1;
                        format!(
                            "{}{}{}",
                            if lead { " " } else { "" },
                            escape(segment),
                            if trail { " " } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(&escape(title)),
            );
        }
        output
    }
}
