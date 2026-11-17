//! Colour tokens a consumer theme declares over its catalogue base theme,
//! applied to the website's colour properties. Catalogue themes contribute
//! none: their website colours are part of the bundled skin.
use {
    crate::publication::{
        render::model::Plan,
        theme::{Builtin, Lineage},
    },
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
};

/// Theme token and the website colour property it sets, in output order.
const TOKENS: [(&str, &str); 4] = [
    ("accent", "--accent"),
    ("inkColor", "--ink"),
    ("mutedColor", "--muted"),
    ("ruleColor", "--line"),
];

pub(in crate::publication::render::website) struct ColourTokens {
    // bounded: at most one declaration per entry of TOKENS.
    declarations: Vec<(&'static str, HexColour)>,
}

struct HexColour(String);

impl HexColour {
    fn try_new(value: &str, token: &str) -> Result<Self> {
        let digits = value.strip_prefix('#').unwrap_or_default();
        ensure!(
            matches!(digits.len(), 3 | 4 | 6 | 8) &&
                digits.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "website theme token {token} must be a #hex colour, not {value}"
        );
        Ok(Self(value.to_ascii_lowercase()))
    }
}

impl ColourTokens {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let lineage =
            Lineage::read(&plan.source, &plan.profile, Builtin::Reader).await?;
        let mut declarations = Vec::new();
        for (token, property) in TOKENS {
            for theme in lineage.themes() {
                if Builtin::from_iri(theme).is_some() {
                    break;
                }
                match plan.source.optional(theme, token).await? {
                    None => continue,
                    Some(Term::Literal(value)) => {
                        declarations.push((
                            property,
                            HexColour::try_new(value.value(), token)?,
                        ));
                        break;
                    },
                    Some(_) => bail!("{theme} {token} must be a literal"),
                }
            }
        }
        Ok(Self { declarations })
    }

    /// A rule for `selector`, or nothing when no token is declared.
    pub fn css(&self, selector: &str) -> String {
        if self.declarations.is_empty() {
            return String::new();
        }
        let body = self
            .declarations
            .iter()
            .map(|(property, colour)| format!("{property}:{}", colour.0))
            .collect::<Vec<_>>()
            .join(";");
        format!("{selector}{{{body}}}")
    }
}
