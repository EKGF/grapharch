//! Validated theme token values in their website (CSS) form. Supplied text
//! never reaches the stylesheet unchecked: every value is a closed keyword, a
//! hexadecimal colour or a font family name of letters, digits, spaces and
//! hyphens.
use {
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// A colour token: `#hex`, `accent`, `accent-tint` or, where the token
/// permits it, `none`.
pub(super) struct ScreenColour(String);

impl ScreenColour {
    pub fn try_new(token: &str, value: &Term, optional: bool) -> Result<Self> {
        let value = plain(token, value)?;
        Ok(Self(match value {
            "accent" => "var(--accent)".into(),
            // The PDF tint lightens the accent by 94%.
            "accent-tint" => {
                "color-mix(in srgb,var(--accent) 6%,var(--paper,#fff))".into()
            },
            "none" if optional => "transparent".into(),
            _ => Self::hex(token, value)?,
        }))
    }

    /// A literal `#hex` colour, as a publisher's accent colour is.
    pub fn hex(token: &str, value: &str) -> Result<String> {
        let digits = value.strip_prefix('#').unwrap_or_default();
        ensure!(
            matches!(digits.len(), 3 | 4 | 6 | 8) &&
                digits.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "website theme token {token} must be a #hex colour, accent or \
             accent-tint, not {value}"
        );
        Ok(value.to_ascii_lowercase())
    }

    pub fn css(&self) -> &str { &self.0 }
}

/// A font family name, quoted, followed by a generic fallback.
pub(super) fn font(
    token: &str,
    value: &Term,
    fallback: &str,
) -> Result<String> {
    let value = plain(token, value)?;
    ensure!(
        !value.is_empty() &&
            value.len() <= 64 &&
            value.trim() == value &&
            value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-')
            }),
        "website theme token {token} must name a font family"
    );
    Ok(format!("\"{value}\",{fallback}"))
}

/// A heading weight as a CSS numeric weight.
pub(super) fn weight(token: &str, value: &Term) -> Result<&'static str> {
    Ok(match plain(token, value)? {
        "regular" => "400",
        "medium" => "500",
        "semibold" => "600",
        "bold" => "700",
        other => {
            bail!(
                "website theme token {token} must be regular, medium, \
                 semibold or bold, not {other}"
            )
        },
    })
}

/// A horizontal alignment keyword.
pub(super) fn align(token: &str, value: &Term) -> Result<&'static str> {
    Ok(match plain(token, value)? {
        "left" => "left",
        "center" => "center",
        "right" => "right",
        other => {
            bail!(
                "website theme token {token} must be left, center or right, \
                 not {other}"
            )
        },
    })
}

fn plain<'a>(token: &str, value: &'a Term) -> Result<&'a str> {
    match value {
        Term::Literal(literal)
            if literal.datatype().as_str() == XSD_STRING &&
                literal.language().is_none() =>
        {
            Ok(literal.value())
        },
        _ => {
            bail!("website theme token {token} must be a plain string literal")
        },
    }
}

#[cfg(test)]
mod tests {
    use {super::*, oxrdf::Literal};

    fn text(value: &str) -> Term { Literal::new_simple_literal(value).into() }

    #[test]
    fn colours_refer_to_the_accent_or_are_hex() -> Result<()> {
        for (value, css) in [
            ("accent", "var(--accent)"),
            ("#ABCDEF", "#abcdef"),
            ("#abc", "#abc"),
            (
                "accent-tint",
                "color-mix(in srgb,var(--accent) 6%,var(--paper,#fff))",
            ),
        ] {
            assert_eq!(
                ScreenColour::try_new("headingColor", &text(value), false)?
                    .css(),
                css
            );
        }
        assert_eq!(
            ScreenColour::try_new("codeFill", &text("none"), true)?.css(),
            "transparent"
        );
        for value in ["none", "red", "#12", "#abcdeg", "accent;}"] {
            assert!(
                ScreenColour::try_new("headingColor", &text(value), false)
                    .is_err(),
                "{value}"
            );
        }
        Ok(())
    }

    #[test]
    fn fonts_weights_and_alignments_are_closed() -> Result<()> {
        assert_eq!(
            font("bodyFont", &text("Liberation Serif"), "serif")?,
            "\"Liberation Serif\",serif"
        );
        assert!(font("bodyFont", &text("A\";}"), "serif").is_err());
        assert_eq!(
            weight("heading1Weight", &text("semibold"))?,
            "600"
        );
        assert!(weight("heading1Weight", &text("900")).is_err());
        assert_eq!(
            align("figureCaptionAlign", &text("right"))?,
            "right"
        );
        assert!(align("figureCaptionAlign", &text("justify")).is_err());
        Ok(())
    }
}
