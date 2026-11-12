//! Resolution of a profile's theme through its `ga:baseTheme` chain.
use {
    super::Builtin,
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

/// The selected theme followed by its bases, most specific first.
pub(crate) struct Lineage {
    // bounded: at most MAX_DEPTH themes.
    themes:  Vec<NamedNode>,
    builtin: Builtin,
}

const MAX_DEPTH: usize = 8;

impl Lineage {
    /// Resolve the profile's `ga:theme`, or the renderer's catalogue default
    /// when the profile selects none.
    pub(crate) async fn read(
        view: &RevisionView,
        profile: &NamedNode,
        default: Builtin,
    ) -> Result<Self> {
        ensure!(
            view.optional(profile, "style").await?.is_none(),
            "profile {profile} uses the removed presentation style string; \
             select an IRI-identified theme with ga:theme"
        );
        let mut current = match view.optional(profile, "theme").await? {
            None => default.iri(),
            Some(Term::NamedNode(theme)) => theme,
            Some(_) => bail!("profile {profile} ga:theme must be a theme IRI"),
        };
        let mut themes = Vec::new();
        loop {
            ensure!(
                !themes.contains(&current),
                "theme {current} has a cyclic ga:baseTheme chain"
            );
            ensure!(
                themes.len() < MAX_DEPTH,
                "theme chain exceeds {MAX_DEPTH} themes"
            );
            // Catalogue themes are engine identities; views of a collection
            // stage need not contain the retained catalogue.
            ensure!(
                Builtin::from_iri(&current).is_some() ||
                    view.has_type(&current, "Theme").await?,
                "{current} is not a declared ga:Theme"
            );
            themes.push(current.clone());
            match view.optional(&current, "baseTheme").await? {
                Some(Term::NamedNode(base)) => current = base,
                None => break,
                Some(_) => bail!("{current} ga:baseTheme must be a theme IRI"),
            }
        }
        let builtin =
            themes.iter().find_map(Builtin::from_iri).ok_or_else(|| {
                anyhow::anyhow!(
                    "theme {} must refine a GraphArch catalogue theme through \
                     ga:baseTheme",
                    themes[0]
                )
            })?;
        Ok(Self { themes, builtin })
    }

    /// The nearest catalogue theme, which selects bundled HTML skins.
    pub(crate) fn builtin(&self) -> Builtin { self.builtin }

    pub(crate) fn selected(&self) -> &NamedNode { &self.themes[0] }

    pub(crate) fn themes(&self) -> &[NamedNode] { &self.themes }
}
