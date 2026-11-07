//! Explicit placement of native model pages in a source-selected sidebar.
use {
    super::bindings::{page_id, q},
    crate::publication::{
        dataset::identity,
        document::maturity::pages::Page,
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Term},
};
pub(super) struct Placement {
    profile: NamedNode,
    parent:  Option<NamedNode>,
    order:   u32,
}
impl Placement {
    pub async fn read(
        view: &RevisionView,
        processor: &NamedNode,
    ) -> Result<Option<Self>> {
        let Some(profile) =
            view.optional(processor, "navigationProfile").await?
        else {
            return Ok(None);
        };
        let Term::NamedNode(profile) = profile else {
            anyhow::bail!("native navigation profile must be an IRI")
        };
        ensure!(
            view.has_type(&profile, "Profile").await?,
            "native navigation profile must be typed Profile"
        );
        let parent = match view
            .optional(processor, "navigationParentPath")
            .await?
        {
            None => None,
            Some(Term::Literal(path)) => {
                let path = path.value();
                ensure!(
                    !path.is_empty() &&
                        path.len() <= 2048 &&
                        path.split('/').all(|segment| {
                            !matches!(segment, "" | "." | "..")
                        }) &&
                        !path.contains('\\'),
                    "native navigation parent path must be relative without \
                     traversal"
                );
                Some(identity("directory-navigation-entry", &[
                    profile.as_str(),
                    path,
                ]))
            },
            _ => {
                anyhow::bail!("native navigation parent path must be a literal")
            },
        };
        let order = match view.optional(processor, "order").await? {
            None => 1_000_000,
            Some(Term::Literal(value)) => value.value().parse::<u32>()?,
            _ => {
                anyhow::bail!(
                    "native navigation order must be a nonnegative integer"
                )
            },
        };
        Ok(Some(Self { profile, parent, order }))
    }

    pub async fn write(
        &self,
        stream: &mut ContributionStream,
        processor: &NamedNode,
        page: &Page,
    ) -> Result<()> {
        let tree = identity("directory-navigation", &[
            self.profile.as_str(),
            "sidebar",
        ]);
        let id = page_id(processor, &page.subject);
        let entry = self.entry(&id);
        for quad in [
            q(&self.profile, "navigation", tree.clone()),
            q(&tree, "type", term("Navigation")),
            q(
                &tree,
                "navigationRegion",
                Literal::new_simple_literal("sidebar"),
            ),
            q(&tree, "navigationEntry", entry.clone()),
            q(&entry, "type", term("NavigationEntry")),
            q(&entry, "page", id),
            q(
                &entry,
                "order",
                Literal::from(
                    if page.parent.is_none() {
                        self.order
                    } else {
                        page.order
                    },
                ),
            ),
        ] {
            stream.write(quad).await?;
        }
        let parent = page
            .parent
            .as_ref()
            .map(|subject| self.entry(&page_id(processor, subject)))
            .or_else(|| self.parent.clone());
        if let Some(parent) = parent {
            stream.write(q(&entry, "parentEntry", parent)).await?;
        }
        Ok(())
    }

    fn entry(&self, page: &NamedNode) -> NamedNode {
        identity("maturity-navigation-entry", &[
            self.profile.as_str(),
            page.as_str(),
        ])
    }
}
