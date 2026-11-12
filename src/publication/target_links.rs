//! Links between targets of one project, written as a bare target IRI. Each
//! resolves to the relative path from the linking page to the linked target's
//! entry file, computed from the declared destinations. One output therefore
//! works wherever it is deployed, as long as the destinations keep their
//! relative layout, which explicit composition (`ga:containsTarget`) allows.
use {
    super::{Renderer, dataset::Dataset, term, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};

/// Registered targets are bounded by the publication definitions.
const TARGETS: usize = 64;

/// Entry files of the project's targets and this target's composition.
#[derive(Clone, Debug, Default)]
pub(crate) struct TargetLinks {
    own:        String,
    files:      BTreeMap<String, String>,
    unrelated:  BTreeSet<String>,
    nested:     Vec<String>,
    containers: Vec<String>,
}

impl TargetLinks {
    /// Read the registered targets of `project` as seen from `target`.
    pub(crate) async fn read(
        dataset: &Dataset,
        view: &RevisionView,
        project: &NamedNode,
        target: &NamedNode,
    ) -> Result<Self> {
        let own = view.text(target, "destination").await?;
        let mut files = BTreeMap::new();
        let mut rows = view.objects(project, "target").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                files.len() < TARGETS,
                "{project} registers more than {TARGETS} targets"
            );
            let Term::NamedNode(registered) = &row["value"] else {
                bail!("{project} ga:target must be an IRI")
            };
            let destination = view.text(registered, "destination").await?;
            let entry = if view.renderer(registered).await? == Renderer::Typst {
                "document.pdf"
            } else {
                "index.html"
            };
            files.insert(
                registered.as_str().to_owned(),
                format!("{destination}/{entry}"),
            );
        }
        let pattern = view.pattern(&format!("?target a {}", term("Target")), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?target WHERE {{ {pattern} }} LIMIT {}",
                TARGETS * 4 + 1
            ))
            .await?;
        let mut unrelated = BTreeSet::new();
        while let Some(row) = rows.next().await? {
            ensure!(
                unrelated.len() < TARGETS * 4,
                "definitions declare more than {} targets",
                TARGETS * 4
            );
            if let Term::NamedNode(declared) = &row["target"] &&
                !files.contains_key(declared.as_str())
            {
                unrelated.insert(declared.as_str().to_owned());
            }
        }
        let mut nested = Vec::new();
        let mut rows = view.objects(target, "containsTarget").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(contained) = &row["value"] else {
                bail!("{target} ga:containsTarget must be an IRI")
            };
            ensure!(
                files.contains_key(contained.as_str()),
                "{target} contains {contained}, which {project} does not \
                 register"
            );
            let destination = view.text(contained, "destination").await?;
            let Some(relative) = destination
                .strip_prefix(&own)
                .and_then(|rest| rest.strip_prefix('/'))
            else {
                bail!(
                    "contained target {contained} destination {destination} \
                     must lie beneath {target} destination {own}"
                )
            };
            nested.push(relative.to_owned());
        }
        nested.sort();
        // bounded: at most TARGETS registered containers.
        let mut containers = Vec::new();
        let mut rows = view.objects(project, "target").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(registered) = &row["value"] else {
                bail!("{project} ga:target must be an IRI")
            };
            let mut contained =
                view.objects(registered, "containsTarget").await?;
            while let Some(value) = contained.next().await? {
                if value["value"] == Term::NamedNode(target.clone()) {
                    containers
                        .push(view.text(registered, "destination").await?);
                }
            }
        }
        Ok(Self { own, files, unrelated, nested, containers })
    }

    /// Whether `destination` belongs to a target that declares it contains
    /// this one.
    pub(crate) fn contained_by(&self, destination: &str) -> bool {
        self.containers
            .iter()
            .any(|container| container == destination)
    }

    /// Destinations of contained targets, relative to this target's.
    pub(crate) fn nested(&self) -> &[String] { &self.nested }

    /// The relative URL of target `iri` from `page`, a path beneath this
    /// target's destination. `None` when `iri` is not a target; an error when
    /// it names a target the project does not register.
    pub(crate) fn resolve(
        &self,
        iri: &str,
        page: &str,
    ) -> Result<Option<String>> {
        let Some(file) = self.files.get(iri) else {
            ensure!(
                !self.unrelated.contains(iri),
                "link to target {iri}, which the project does not register"
            );
            return Ok(None);
        };
        let from = format!("{}/{page}", self.own);
        Ok(Some(relative(&from, file)))
    }
}

/// Relative URL from file `from` to file `to`, both beneath one output root.
fn relative(from: &str, to: &str) -> String {
    let from: Vec<&str> = from.split('/').collect();
    let to: Vec<&str> = to.split('/').collect();
    let directories = &from[..from.len() - 1];
    let common = directories
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut path = "../".repeat(directories.len() - common);
    path.push_str(&to[common..].join("/"));
    path
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_paths_follow_the_destination_layout() {
        assert_eq!(
            relative("site/spec/main/index.html", "site/index.html"),
            "../../index.html"
        );
        assert_eq!(
            relative("site/index.html", "site/pdf/document.pdf"),
            "pdf/document.pdf"
        );
        assert_eq!(
            relative("site/guide/index.html", "spec/index.html"),
            "../../spec/index.html"
        );
        assert_eq!(
            relative("a/index.html", "a/index.html"),
            "index.html"
        );
    }
}
