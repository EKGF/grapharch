//! `book.toml`, read as data: title, description, content root and the
//! repository link. Everything executable or presentational becomes a warning.
use {
    super::warning::MdBookWarning,
    anyhow::{Result, ensure},
    toml::{Table, Value},
};

/// The largest accepted `book.toml`.
pub(super) const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_TEXT_BYTES: usize = 4096;

/// The parts of `book.toml` that GraphArch publishes.
#[derive(Debug)]
pub(super) struct BookConfig {
    pub title:       String,
    pub description: Option<String>,
    /// The content directory relative to the book root (`src` by default).
    pub source:      String,
    pub repository:  Option<url::Url>,
    // bounded: one per table entry of a 64 KiB file.
    pub warnings:    Vec<MdBookWarning>,
}

impl BookConfig {
    pub(super) fn parse(body: &str) -> Result<Self> {
        ensure!(
            body.len() as u64 <= MAX_CONFIG_BYTES,
            "book.toml exceeds 64 KiB"
        );
        let table: Table = toml::from_str(body)?;
        let book = match table.get("book") {
            Some(Value::Table(book)) => book.clone(),
            Some(_) => anyhow::bail!("book.toml [book] must be a table"),
            None => Table::new(),
        };
        let mut warnings = Vec::new();
        for (key, value) in &table {
            match key.as_str() {
                "book" | "build" | "rust" => {},
                "preprocessor" => {
                    for name in Self::names(value, "preprocessor")? {
                        warnings
                            .push(MdBookWarning::PreprocessorNotExecuted(name));
                    }
                },
                "output" => Self::outputs(value, &mut warnings)?,
                other => {
                    warnings.push(MdBookWarning::PresentationSubstitution(
                        other.to_owned(),
                    ))
                },
            }
        }
        let repository = Self::repository(&table)?;
        Ok(Self {
            title: Self::text(&book, "title")?
                .ok_or_else(|| anyhow::anyhow!("book.toml requires title"))?,
            description: Self::text(&book, "description")?,
            source: Self::text(&book, "src")?.unwrap_or_else(|| "src".into()),
            repository,
            warnings,
        })
    }

    fn names(value: &Value, section: &str) -> Result<Vec<String>> {
        let Value::Table(entries) = value else {
            anyhow::bail!("book.toml [{section}] must be a table")
        };
        Ok(entries.keys().cloned().collect())
    }

    fn outputs(value: &Value, warnings: &mut Vec<MdBookWarning>) -> Result<()> {
        let Value::Table(outputs) = value else {
            anyhow::bail!("book.toml [output] must be a table")
        };
        for (name, options) in outputs {
            if name != "html" {
                warnings.push(MdBookWarning::OutputNotProduced(name.clone()));
                continue;
            }
            let Value::Table(options) = options else {
                anyhow::bail!("book.toml [output.html] must be a table")
            };
            for key in options.keys() {
                if key != "git-repository-url" {
                    warnings.push(MdBookWarning::PresentationSubstitution(
                        format!("output.html.{key}"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn repository(table: &Table) -> Result<Option<url::Url>> {
        let Some(value) = table
            .get("output")
            .and_then(|output| output.get("html"))
            .and_then(|html| html.get("git-repository-url"))
        else {
            return Ok(None);
        };
        let value = value.as_str().ok_or_else(|| {
            anyhow::anyhow!("git-repository-url must be text")
        })?;
        let url = url::Url::parse(value)?;
        ensure!(
            matches!(url.scheme(), "http" | "https"),
            "git-repository-url must use HTTP(S)"
        );
        Ok(Some(url))
    }

    fn text(table: &Table, key: &str) -> Result<Option<String>> {
        table
            .get(key)
            .map(|value| {
                let text = value.as_str().ok_or_else(|| {
                    anyhow::anyhow!("book.{key} must be text")
                })?;
                ensure!(
                    !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES,
                    "book.{key} must contain 1–4096 bytes"
                );
                Ok(text.to_owned())
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_data_and_reports_programs_without_running_them() -> Result<()> {
        let config = BookConfig::parse(
            "[book]\ntitle = \"T\"\n[preprocessor.katex]\ncommand = \
             \"rm -rf /\"\n[output.html]\ngit-repository-url = \
             \"https://example.org/r\"\nmathjax-support = true\n[output.pdf]\n",
        )?;
        assert_eq!(config.title, "T");
        assert_eq!(config.source, "src");
        assert_eq!(
            config.repository.map(String::from).as_deref(),
            Some("https://example.org/r")
        );
        assert!(config.warnings.contains(
            &MdBookWarning::PreprocessorNotExecuted("katex".into())
        ));
        assert!(
            config
                .warnings
                .contains(&MdBookWarning::OutputNotProduced("pdf".into()))
        );
        assert!(config.warnings.contains(
            &MdBookWarning::PresentationSubstitution(
                "output.html.mathjax-support".into()
            )
        ));
        Ok(())
    }

    #[test]
    fn requires_a_title_and_https_repository() {
        assert!(BookConfig::parse("[book]\n").is_err());
        assert!(
            BookConfig::parse(
                "[book]\ntitle = \"T\"\n[output.html]\ngit-repository-url = \
                 \"file:///etc\"\n"
            )
            .is_err()
        );
    }
}
