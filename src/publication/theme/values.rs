//! Closed value syntaxes for theme tokens. Each value is validated in Rust and
//! mapped to a Typst value; no supplied text is evaluated as Typst source.
use {
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";
const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// The syntax accepted by one token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `#RRGGBB`, `accent`, or `accent-tint`.
    Color,
    /// A [`Kind::Color`] or `none`.
    OptionalColor,
    /// An absolute or font-relative length: `pt`, `mm`, `cm`, `in`, `em`.
    Length,
    /// A [`Kind::Length`] or a percentage.
    Relative,
    /// A [`Kind::Length`] or a fraction of the remaining space, `fr`.
    Spacing,
    Bool,
    Text,
    Font,
    Weight,
    Align,
    Paper,
    Numbering,
    Depth,
    /// Text that must contain the named placeholder.
    Pattern(&'static str),
}

impl Kind {
    /// Validate a token value and return its Typst expression.
    pub(crate) fn typst(self, token: &str, value: &Term) -> Result<String> {
        match self {
            Self::Bool => Ok(boolean(token, value)?.to_string()),
            Self::Depth => {
                let depth = integer(token, value)?;
                ensure!(
                    (1..=8).contains(&depth),
                    "theme token {token} must be between one and eight"
                );
                Ok(depth.to_string())
            },
            _ => self.literal(token, string(token, value)?),
        }
    }

    fn literal(self, token: &str, value: &str) -> Result<String> {
        match self {
            Self::Color | Self::OptionalColor => {
                color(token, value, self == Self::OptionalColor)
            },
            Self::Length => length(token, value, &[]),
            Self::Relative => length(token, value, &["%"]),
            Self::Spacing => length(token, value, &["fr"]),
            Self::Text => {
                ensure!(
                    !value.trim().is_empty() && value.len() <= 256,
                    "theme token {token} must be nonempty text of at most 256 \
                     bytes"
                );
                Ok(quote(value))
            },
            Self::Pattern(placeholder) => {
                ensure!(
                    value.contains(placeholder) && value.len() <= 256,
                    "theme token {token} must contain {placeholder}"
                );
                Ok(quote(value))
            },
            Self::Font => {
                ensure!(
                    !value.is_empty() &&
                        value.len() <= 64 &&
                        value.trim() == value &&
                        value.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() ||
                                matches!(byte, b' ' | b'-')
                        }),
                    "theme token {token} must name a font family"
                );
                Ok(quote(value))
            },
            Self::Weight => {
                closed(token, value, &[
                    "regular", "medium", "semibold", "bold",
                ])
                .map(quote)
            },
            Self::Align => {
                closed(token, value, &["left", "center", "right"])
                    .map(str::to_owned)
            },
            Self::Paper => {
                closed(token, value, &[
                    "a3",
                    "a4",
                    "a5",
                    "us-letter",
                    "us-legal",
                ])
                .map(quote)
            },
            Self::Numbering => {
                closed(token, value, &["1", "i", "I", "a", "A"]).map(quote)
            },
            Self::Bool | Self::Depth => unreachable!("typed literals"),
        }
    }
}

fn closed<'a>(
    token: &str,
    value: &'a str,
    allowed: &[&str],
) -> Result<&'a str> {
    ensure!(
        allowed.contains(&value),
        "theme token {token} must be one of {}",
        allowed.join(", ")
    );
    Ok(value)
}

fn color(token: &str, value: &str, optional: bool) -> Result<String> {
    Ok(match value {
        "accent" => "ga-accent".into(),
        "accent-tint" => "ga-accent.lighten(94%)".into(),
        "none" if optional => "none".into(),
        _ => format!("rgb({})", quote(&hex(token, value)?)),
    })
}

/// A validated `#RRGGBB` sRGB color, lower-cased.
pub(crate) fn hex(token: &str, value: &str) -> Result<String> {
    ensure!(
        value.len() == 7 &&
            value.starts_with('#') &&
            value[1..].bytes().all(|byte| byte.is_ascii_hexdigit()),
        "theme token {token} must be #RRGGBB, accent or accent-tint"
    );
    Ok(value.to_ascii_lowercase())
}

fn length(token: &str, value: &str, extra: &[&str]) -> Result<String> {
    let unit = ["pt", "mm", "cm", "in", "em"]
        .iter()
        .chain(extra)
        .find(|unit| value.ends_with(*unit))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "theme token {token} requires a unit: pt, mm, cm, in, em{}",
                extra
                    .iter()
                    .map(|unit| format!(", {unit}"))
                    .collect::<String>()
            )
        })?;
    let number = &value[..value.len() - unit.len()];
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    ensure!(
        (1..=4).contains(&whole.len()) &&
            whole.bytes().all(|byte| byte.is_ascii_digit()) &&
            fraction.len() <= 3 &&
            fraction.bytes().all(|byte| byte.is_ascii_digit()) &&
            (!number.contains('.') || !fraction.is_empty()),
        "theme token {token} must be a nonnegative decimal length"
    );
    Ok(value.to_owned())
}

fn string<'a>(token: &str, value: &'a Term) -> Result<&'a str> {
    match value {
        Term::Literal(literal)
            if literal.datatype().as_str() == XSD_STRING &&
                literal.language().is_none() =>
        {
            Ok(literal.value())
        },
        _ => bail!("theme token {token} must be a plain string literal"),
    }
}

pub(crate) fn boolean(token: &str, value: &Term) -> Result<bool> {
    match value {
        Term::Literal(literal)
            if literal.datatype().as_str() == XSD_BOOLEAN =>
        {
            match literal.value() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => bail!("theme token {token} must be an xsd:boolean"),
            }
        },
        _ => bail!("theme token {token} must be an xsd:boolean"),
    }
}

pub(crate) fn integer(token: &str, value: &Term) -> Result<u32> {
    match value {
        Term::Literal(literal)
            if literal.datatype().as_str() == XSD_INTEGER =>
        {
            Ok(literal.value().parse::<u32>()?)
        },
        _ => bail!("theme token {token} must be an xsd:integer"),
    }
}

/// A Typst string literal.
pub(crate) fn quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

#[cfg(test)]
mod tests {
    use {super::*, oxrdf::Literal};

    fn text(value: &str) -> Term { Literal::new_simple_literal(value).into() }

    #[test]
    fn values_map_to_typst_without_evaluating_supplied_text() -> Result<()> {
        assert_eq!(
            Kind::Color.typst("ink", &text("#4E80BC"))?,
            "rgb(\"#4e80bc\")"
        );
        assert_eq!(
            Kind::Color.typst("ink", &text("accent"))?,
            "ga-accent"
        );
        assert_eq!(
            Kind::OptionalColor.typst("fill", &text("none"))?,
            "none"
        );
        assert!(Kind::Color.typst("ink", &text("none")).is_err());
        assert_eq!(
            Kind::Length.typst("size", &text("0.79in"))?,
            "0.79in"
        );
        assert!(Kind::Length.typst("size", &text("10")).is_err());
        assert!(Kind::Length.typst("size", &text("1.pt")).is_err());
        assert!(Kind::Length.typst("size", &text("30%")).is_err());
        assert_eq!(
            Kind::Relative.typst("descent", &text("30%"))?,
            "30%"
        );
        assert_eq!(Kind::Spacing.typst("space", &text("1fr"))?, "1fr");
        assert!(Kind::Font.typst("font", &text("Arial\"); evil()")).is_err());
        assert_eq!(
            Kind::Text.typst("title", &text("Table \"of\" Contents"))?,
            "\"Table \\\"of\\\" Contents\""
        );
        assert!(Kind::Paper.typst("paper", &text("letter")).is_err());
        assert!(Kind::Bool.typst("flag", &text("true")).is_err());
        assert_eq!(
            Kind::Bool.typst("flag", &Literal::from(true).into())?,
            "true"
        );
        assert!(
            Kind::Pattern("{letter}")
                .typst("annex", &text("Annex:"))
                .is_err()
        );
        Ok(())
    }
}
