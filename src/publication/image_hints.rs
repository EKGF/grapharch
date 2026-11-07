//! Typed image presentation hints retain ordinary author-provided titles.
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};
const PREFIX: &str = "grapharch:image ";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ColorScheme {
    Light,
    Dark,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImageHints {
    #[serde(default)]
    pub interactive_svg: bool,
    #[serde(default)]
    pub color_scheme:    Option<ColorScheme>,
    #[serde(default)]
    pub title:           Option<String>,
}
impl ImageHints {
    pub fn parse(title: &str) -> Result<Self> {
        ensure!(
            title.len() <= 8192,
            "image title exceeds 8192 bytes"
        );
        if let Some(encoded) = title.strip_prefix(PREFIX) {
            return Ok(serde_json::from_str(encoded)?);
        }
        let Some(tokens) = title.strip_prefix("grapharch:") else {
            return Ok(Self {
                title: (!title.is_empty()).then(|| title.into()),
                ..Self::default()
            });
        };
        let mut hints = Self::default();
        for token in tokens.split(';') {
            match token {
                "interactive-svg" => {
                    ensure!(
                        !hints.interactive_svg,
                        "duplicate image interaction hint"
                    );
                    hints.interactive_svg = true;
                },
                "only-light" | "only-dark" => {
                    ensure!(
                        hints.color_scheme.is_none(),
                        "conflicting image color scheme hints"
                    );
                    hints.color_scheme = Some(
                        if token == "only-light" {
                            ColorScheme::Light
                        } else {
                            ColorScheme::Dark
                        },
                    );
                },
                _ => {
                    anyhow::bail!(
                        "unsupported image presentation hint: {token}"
                    )
                },
            }
        }
        Ok(hints)
    }

    pub fn encoded(&self) -> Result<String> {
        let encoded = format!("{PREFIX}{}", serde_json::to_string(self)?);
        ensure!(
            encoded.len() <= 8192,
            "image hints exceed 8192 bytes"
        );
        Ok(encoded)
    }
}
#[cfg(test)]
mod tests {
    use super::{ColorScheme, ImageHints};
    #[test]
    fn image_hints_keep_titles_and_compose_presentation_axes() {
        let mut hints = ImageHints::parse("Original copyright title").unwrap();
        hints.interactive_svg = true;
        hints.color_scheme = Some(ColorScheme::Dark);
        assert_eq!(
            ImageHints::parse(&hints.encoded().unwrap()).unwrap(),
            hints
        );
        let shorthand =
            ImageHints::parse("grapharch:interactive-svg;only-light").unwrap();
        assert!(shorthand.interactive_svg);
        assert_eq!(shorthand.color_scheme, Some(ColorScheme::Light));
        assert!(ImageHints::parse("grapharch:only-light;only-dark").is_err());
        assert!(ImageHints::parse("grapharch:onclick=alert(1)").is_err());
        assert!(
            ImageHints::parse("grapharch:image {\"unknown\":true}").is_err()
        );
    }
}
