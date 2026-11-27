//! Where website chrome is declared: the profile, then each theme of its
//! lineage from the most specific. The most specific declaration of a chrome
//! property applies, so a house theme can carry identity, header links and a
//! footer to every website selecting it, and a profile can replace them.
use {
    crate::publication::{
        render::model::Plan,
        theme::{Builtin, Lineage},
    },
    anyhow::Result,
    oxrdf::NamedNode,
};

pub(in crate::publication::render::website) struct ChromeSources {
    profile: NamedNode,
    // bounded: the theme lineage, at most eight themes.
    themes:  Vec<NamedNode>,
}

impl ChromeSources {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let lineage =
            Lineage::read(&plan.source, &plan.profile, Builtin::Reader).await?;
        Ok(Self {
            profile: plan.profile.clone(),
            themes:  lineage.themes().to_vec(),
        })
    }

    pub fn profile(&self) -> &NamedNode { &self.profile }

    /// The most specific subject declaring `property`.
    pub async fn nearest(
        &self,
        plan: &Plan,
        property: &str,
    ) -> Result<Option<&NamedNode>> {
        for subject in std::iter::once(&self.profile).chain(&self.themes) {
            if plan
                .source
                .objects(subject, property)
                .await?
                .next()
                .await?
                .is_some()
            {
                return Ok(Some(subject));
            }
        }
        Ok(None)
    }

    /// The most specific theme declaring `property`, ignoring the profile.
    pub async fn nearest_theme(
        &self,
        plan: &Plan,
        property: &str,
    ) -> Result<Option<&NamedNode>> {
        for theme in &self.themes {
            if plan
                .source
                .objects(theme, property)
                .await?
                .next()
                .await?
                .is_some()
            {
                return Ok(Some(theme));
            }
        }
        Ok(None)
    }
}
