//! Sidebar labels that identify their page even when two listed pages share
//! a title, such as a repository README and a binding's README.
use {
    super::{Catalog, Page},
    std::collections::{BTreeMap, HashMap},
};

/// How a repeated title is told apart.
#[derive(Clone, Copy)]
enum Qualifier {
    /// The page's source directory; a page at the root keeps its title.
    Directory,
    /// The page's whole source path.
    SourcePath,
}

impl Page {
    /// Whether the page appears in the sidebar.
    pub(super) fn listed(&self) -> bool { !self.asset && !self.hidden }

    fn qualified(&self, qualifier: Qualifier) -> String {
        let suffix = match qualifier {
            Qualifier::Directory => {
                self.source
                    .rsplit_once('/')
                    .map(|(parent, _)| parent)
                    .filter(|parent| !parent.is_empty())
            },
            Qualifier::SourcePath => Some(self.source.as_str()),
        };
        match suffix {
            Some(suffix) => format!("{} ({suffix})", self.title),
            None => self.title.clone(),
        }
    }

    /// Qualify every listed page whose label another listed page repeats:
    /// first by source directory, then, where that still collides, by
    /// source path. A unique title stays as it is.
    fn qualify_labels(pages: &mut BTreeMap<String, Page>) {
        for qualifier in [Qualifier::Directory, Qualifier::SourcePath] {
            // bounded: one entry per listed page, within the page budget.
            let mut counts: HashMap<String, usize> = HashMap::new();
            for page in pages.values().filter(|page| page.listed()) {
                *counts.entry(page.navigation_label.clone()).or_default() += 1;
            }
            for page in pages.values_mut().filter(|page| page.listed()) {
                if counts[&page.navigation_label] > 1 {
                    page.navigation_label = page.qualified(qualifier);
                }
            }
        }
    }
}

impl Catalog {
    pub(super) fn qualify_navigation_labels(&mut self) {
        Page::qualify_labels(&mut self.pages);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::Page,
        oxrdf::NamedNode,
        std::collections::{BTreeMap, HashSet},
    };

    fn page(id: &str, title: &str, source: &str) -> (String, Page) {
        (id.to_owned(), Page {
            id:               NamedNode::new_unchecked(id),
            title:            title.to_owned(),
            navigation_label: title.to_owned(),
            source:           source.to_owned(),
            route:            String::new(),
            asset:            false,
            binary:           false,
            hidden:           false,
            order:            500,
            parent:           None,
            parent_title:     None,
            headings:         HashSet::new(),
        })
    }

    fn labels(pages: &[(&str, &str, &str)]) -> Vec<String> {
        let mut pages: BTreeMap<_, _> = pages
            .iter()
            .map(|(id, title, source)| page(id, title, source))
            .collect();
        Page::qualify_labels(&mut pages);
        pages
            .into_values()
            .map(|page| page.navigation_label)
            .collect()
    }

    #[test]
    fn a_binding_readme_named_like_the_project_names_its_directory() {
        assert_eq!(
            labels(&[
                ("urn:a", "dotenvage", "README.md"),
                ("urn:b", "dotenvage", "python/README.md"),
                ("urn:c", "@dotenvage/node", "npm/README.md"),
            ]),
            ["dotenvage", "dotenvage (python)", "@dotenvage/node"]
        );
    }

    #[test]
    fn namesakes_in_one_directory_name_their_source_paths() {
        assert_eq!(
            labels(&[
                ("urn:a", "Guide", "docs/a.md"),
                ("urn:b", "Guide", "docs/b.md"),
            ]),
            ["Guide (docs/a.md)", "Guide (docs/b.md)"]
        );
    }
}
