//! Theme tokens applied to a website. A website skin, the reader or an EKGF
//! catalogue theme, owns its screen values. Every theme refining a skin,
//! whether a catalogue theme such as `ga:SpecificationTheme` or a consumer
//! theme, overrides them with the screen tokens it declares; the most specific
//! declaration wins. Print tokens are validated by PDF renderers only.
use {
    super::{
        screen_rules::{Declaration, Effect, SCREEN_TOKENS, ScreenToken},
        screen_value::ScreenColour,
    },
    crate::publication::{
        NAMESPACE,
        render::model::Plan,
        theme::{Builtin, Lineage, boolean},
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

const MAX_STATEMENTS: usize = 256;

pub(in crate::publication::render::website) struct ThemeTokens {
    // bounded: at most one value per screen token.
    values: Vec<(&'static ScreenToken, Term)>,
    accent: Option<String>,
}

impl ThemeTokens {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let lineage =
            Lineage::read(&plan.source, &plan.profile, Builtin::Reader).await?;
        let mut values: Vec<(&'static ScreenToken, Term)> = Vec::new();
        for theme in lineage.themes() {
            if Builtin::from_iri(theme).is_some_and(Builtin::website_skin) {
                break;
            }
            for (token, value) in Self::declared(plan, theme).await? {
                if !values.iter().any(|(known, _)| known.name == token.name) {
                    values.push((token, value));
                }
            }
        }
        let mut tokens = Self { values, accent: None };
        if tokens.flag("authorityAccent")? {
            tokens.accent = Self::publisher_accent(plan).await?;
        }
        Ok(tokens)
    }

    /// The screen tokens one theme declares itself.
    async fn declared(
        plan: &Plan,
        theme: &NamedNode,
    ) -> Result<Vec<(&'static ScreenToken, Term)>> {
        // bounded: at most MAX_STATEMENTS statements of one theme.
        let mut declared = Vec::new();
        let mut rows = plan.source.statements(theme).await?;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= MAX_STATEMENTS,
                "theme {theme} exceeds {MAX_STATEMENTS} statements"
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                bail!("theme predicate must be an IRI")
            };
            let Some(local) = predicate.as_str().strip_prefix(NAMESPACE) else {
                continue;
            };
            if let Some(token) =
                SCREEN_TOKENS.iter().find(|token| token.name == local)
            {
                ensure!(
                    !declared.iter().any(
                        |(known, _): &(&ScreenToken, Term)| known.name == local
                    ),
                    "theme {theme} declares ga:{local} more than once"
                );
                declared.push((token, row["value"].clone()));
            }
        }
        Ok(declared)
    }

    /// The target publisher's `ga:accentColor`, when it declares one.
    async fn publisher_accent(plan: &Plan) -> Result<Option<String>> {
        let publisher =
            match plan.source.optional(&plan.target, "publisher").await? {
                None => return Ok(None),
                Some(Term::NamedNode(publisher)) => publisher,
                Some(_) => bail!("website publisher must be an IRI"),
            };
        match plan.source.optional(&publisher, "accentColor").await? {
            None => Ok(None),
            Some(Term::Literal(colour)) => {
                Ok(Some(ScreenColour::hex(
                    "accentColor",
                    colour.value(),
                )?))
            },
            Some(_) => bail!("publisher accentColor must be a literal"),
        }
    }

    /// Whether a boolean screen token is declared true.
    pub fn flag(&self, name: &str) -> Result<bool> {
        match self.values.iter().find(|(token, _)| token.name == name) {
            Some((token, value)) => boolean(token.name, value),
            None => Ok(false),
        }
    }

    /// The stylesheet for `root`, the skin's root selector: custom properties
    /// first, then the rules of the remaining tokens.
    pub fn css(&self, root: &str) -> Result<String> {
        let mut properties = Vec::new();
        let mut rules = String::new();
        for (token, value) in &self.values {
            match token.declaration(value)? {
                Declaration::Property(property, css) => {
                    let css = match (property, &self.accent) {
                        ("--accent", Some(accent)) => accent.clone(),
                        _ => css,
                    };
                    properties.push(format!("{property}:{css}"));
                },
                Declaration::Rule(selector, declaration) => {
                    let selector = if selector.is_empty() {
                        root.to_owned()
                    } else {
                        format!("{root} {selector}")
                    };
                    rules.push_str(&format!("{selector}{{{declaration}}}"));
                },
                Declaration::None => {},
            }
        }
        if let Some(accent) = &self.accent &&
            !self.values.iter().any(|(token, _)| {
                matches!(token.effect, Effect::Colour {
                    property: "--accent",
                    ..
                })
            })
        {
            properties.push(format!("--accent:{accent}"));
        }
        if properties.is_empty() {
            return Ok(rules);
        }
        Ok(format!(
            "{root}{{{}}}{rules}",
            properties.join(";")
        ))
    }
}
