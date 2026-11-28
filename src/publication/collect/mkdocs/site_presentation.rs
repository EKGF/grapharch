//! The RDF presentation of an imported MkDocs website: the theme selected
//! through the mapping table, refined by palette colours and mapped screen
//! tokens; profile switches for Material's layout features; and the footer's
//! social links and copyright.
use {
    super::{config::Config, lit, mapping::Mappings, material::Feature, q},
    crate::publication::{dataset::identity, term, writer::ContributionStream},
    anyhow::Result,
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

const RIGHTS: &str = "http://purl.org/dc/terms/rights";
const TITLE: &str = "http://purl.org/dc/terms/title";

pub(super) struct SitePresentation<'a> {
    pub config:  &'a Config,
    pub project: &'a NamedNode,
    pub profile: &'a NamedNode,
}

impl SitePresentation<'_> {
    pub(super) async fn write(
        &self,
        stream: &mut ContributionStream,
    ) -> Result<()> {
        // bounded: a few dozen presentation statements.
        let mut quads = self.theme()?;
        quads.extend(self.switches());
        quads.extend(self.footer());
        for quad in quads {
            stream.write(quad).await?;
        }
        Ok(())
    }

    /// A theme of this website refining the mapped theme, when the table or
    /// the palette contributes anything.
    fn theme(&self) -> Result<Vec<Quad>> {
        let Self { config, project, profile } = self;
        let table = Mappings::embedded()?;
        let selection =
            table.select(config.theme_name.as_deref(), &config.plugins);
        let primary = config
            .material
            .primary
            .as_deref()
            .and_then(|name| table.color(name));
        if selection.theme.is_none() &&
            selection.tokens.is_empty() &&
            primary.is_none()
        {
            return Ok(Vec::new());
        }
        let theme = identity("mkdocs-theme", &[project.as_str()]);
        let base = selection.theme.unwrap_or_else(|| {
            crate::publication::theme::Builtin::Reader.iri()
        });
        let mut quads = vec![
            q(profile, "theme", theme.clone()),
            q(&theme, "type", term("Theme")),
            q(&theme, "baseTheme", base),
        ];
        for (predicate, value) in selection.tokens {
            quads.push(Quad::new(
                theme.clone(),
                predicate,
                value,
                GraphName::DefaultGraph,
            ));
        }
        if let Some(color) = primary {
            quads.push(q(&theme, "linkColor", lit(color)));
            quads.push(q(&theme, "accent", lit(color)));
        }
        Ok(quads)
    }

    /// Profile switches: Material shows breadcrumbs, previous/next links and
    /// its credit only when the configuration enables them.
    fn switches(&self) -> Vec<Quad> {
        let Self { config, profile, .. } = self;
        let material = &config.material;
        let mut quads = vec![
            q(
                profile,
                "breadcrumbs",
                Literal::from(material.has(Feature::Path)),
            ),
            q(
                profile,
                "pageNeighbors",
                Literal::from(material.has(Feature::Footer)),
            ),
            q(
                profile,
                "generatorCredit",
                Literal::from(material.generator),
            ),
        ];
        if config.theme_name.as_deref() == Some("material") {
            quads.push(q(
                profile,
                "primaryNavigation",
                Literal::from(material.has(Feature::Tabs)),
            ));
        }
        if material.has(Feature::TocIntegrate) {
            quads.push(q(
                profile,
                "outlineInSidebar",
                Literal::from(true),
            ));
        }
        if material.has(Feature::Sections) {
            quads.push(q(profile, "sidebarSections", Literal::from(true)));
        }
        if config.smart_symbols {
            quads.push(q(profile, "smartSymbols", Literal::from(true)));
        }
        quads
    }

    fn footer(&self) -> Vec<Quad> {
        let Self { config, profile, .. } = self;
        let mut quads = Vec::new(); // bounded: MAX_SOCIAL links.
        if let Some(copyright) = &config.material.copyright {
            quads.push(Quad::new(
                (*profile).clone(),
                NamedNode::new_unchecked(RIGHTS),
                lit(copyright),
                GraphName::DefaultGraph,
            ));
        }
        for (order, social) in config.material.social.iter().enumerate() {
            let link = identity("mkdocs-social", &[
                profile.as_str(),
                &order.to_string(),
            ]);
            quads.extend([
                q(profile, "socialLink", link.clone()),
                q(&link, "url", lit(&social.link)),
                q(&link, "iconName", lit(&social.icon)),
                q(&link, "order", Literal::from(order as u32 + 1)),
            ]);
            if let Some(title) = &social.title {
                quads.push(Quad::new(
                    link,
                    NamedNode::new_unchecked(TITLE),
                    lit(title),
                    GraphName::DefaultGraph,
                ));
            }
        }
        quads
    }
}
