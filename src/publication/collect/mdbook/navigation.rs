//! The `SUMMARY.md` outline as `ga:Navigation`, in the same shape the MkDocs
//! importer produces.
use {
    super::{
        collector::{q, title},
        summary::Summary,
    },
    crate::publication::{dataset::identity, term, writer::ContributionStream},
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode},
    std::path::Path,
};

impl Summary {
    /// Write the sidebar and primary navigation trees of `profile`.
    pub(super) async fn write(
        &self,
        stream: &mut ContributionStream,
        profile: &NamedNode,
        root: &Path,
    ) -> Result<()> {
        let trees = ["sidebar", "primary"].map(|region| {
            (
                identity("mdbook-navigation", &[profile.as_str(), region]),
                region,
            )
        });
        for (tree, region) in &trees {
            for quad in [
                q(profile, "navigation", tree.clone()),
                q(tree, "type", term("Navigation")),
                q(
                    tree,
                    "navigationRegion",
                    Literal::new_simple_literal(*region),
                ),
                title(tree, "Documentation"),
            ] {
                stream.write(quad).await?;
            }
        }
        let identities: Vec<NamedNode> = (0..self.entries.len())
            .map(|index| {
                identity("mdbook-navigation-entry", &[
                    profile.as_str(),
                    &index.to_string(),
                ])
            })
            .collect(); // bounded: Summary holds at most 512 entries.
        let mut order = vec![0u32; self.entries.len() + 1]; // bounded: same.
        let mut parents = vec![false; self.entries.len()]; // bounded: same.
        for parent in self.entries.iter().filter_map(|entry| entry.parent) {
            parents[parent] = true;
        }
        for (index, (entry, node)) in
            self.entries.iter().zip(&identities).enumerate()
        {
            // A draft chapter has no page yet; mdBook lists it disabled.
            if entry.destination.is_none() && !parents[index] {
                continue;
            }
            let slot = entry.parent.unwrap_or(self.entries.len());
            for (tree, _) in &trees {
                stream
                    .write(q(tree, "navigationEntry", node.clone()))
                    .await?;
            }
            for quad in [
                q(node, "type", term("NavigationEntry")),
                q(node, "order", Literal::from(order[slot])),
                title(node, &entry.title),
            ] {
                stream.write(quad).await?;
            }
            order[slot] += 1;
            if let Some(parent) = entry.parent {
                stream
                    .write(q(node, "parentEntry", identities[parent].clone()))
                    .await?;
            }
            let Some(destination) = &entry.destination else {
                continue;
            };
            let file = tokio::fs::canonicalize(root.join(&destination.path))
                .await
                .map_err(|error| {
                    anyhow::anyhow!(
                        "SUMMARY.md destination {}: {error}",
                        destination.path
                    )
                })?;
            ensure!(
                file.starts_with(root) && file.is_file(),
                "SUMMARY.md destination must be relative to the book source \
                 and a Markdown file: {}",
                destination.path
            );
            stream
                .write(q(
                    node,
                    "sourcePath",
                    Literal::new_simple_literal(&destination.path),
                ))
                .await?;
            if let Some(anchor) = &destination.anchor {
                stream
                    .write(q(
                        node,
                        "headingAnchor",
                        Literal::new_simple_literal(anchor),
                    ))
                    .await?;
            }
        }
        Ok(())
    }
}
