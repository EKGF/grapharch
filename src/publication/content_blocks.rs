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
    Alternative {
        title: String,
    },
    Admonition {
        category:    String,
        title:       String,
        collapsible: bool,
        expanded:    bool,
    },
    End,
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

    pub fn finish(self) -> Result<()> {
        ensure!(self.0.is_empty(), "unclosed content container");
        Ok(())
    }
}
