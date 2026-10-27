//! PDF skins are selected only from the pinned target and profile definitions.
use {
    super::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::Term,
};

pub(super) struct Style {
    pub name:   &'static str,
    pub accent: String,
}
impl Style {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let requested =
            match plan.source.optional(&plan.profile, "style").await? {
                Some(Term::Literal(style)) => Some(style.value().to_owned()),
                None => None,
                _ => anyhow::bail!("PDF profile style must be a literal"),
            };
        ensure!(
            requested
                .as_deref()
                .is_none_or(|v| matches!(v, "specification" | "reader")),
            "unsupported PDF presentation style {}",
            requested.as_deref().unwrap_or_default()
        );
        let name = if matches!(
            plan.renderer.as_str(),
            "specification" | "bookshelf"
        ) || requested.as_deref() == Some("specification")
        {
            "specification"
        } else {
            "reader"
        };
        let mut accent = if name == "reader" {
            "#116659".into()
        } else {
            "#005a9c".into()
        };
        if name == "specification" {
            match plan.source.optional(&plan.target, "publisher").await? {
                Some(Term::NamedNode(publisher)) => {
                    accent = match plan
                        .source
                        .optional(&publisher, "accentColor")
                        .await?
                    {
                        Some(Term::Literal(color)) => color.value().to_owned(),
                        None => accent,
                        _ => {
                            anyhow::bail!(
                                "authority accentColor must be a literal"
                            )
                        },
                    };
                },
                None => {},
                _ => anyhow::bail!("PDF publisher must be an IRI"),
            }
        }
        ensure!(
            accent.len() == 7 &&
                accent.starts_with('#') &&
                accent[1..].bytes().all(|c| c.is_ascii_hexdigit()),
            "PDF accentColor must be #RRGGBB"
        );
        Ok(Self { name, accent })
    }
}
