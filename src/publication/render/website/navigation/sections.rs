//! Section ownership follows explicit primary navigation and existing page
//! ancestry.
use {
    super::{Catalog, Entry, SiteLink, Tree},
    anyhow::{Result, ensure},
    std::collections::BTreeMap,
};

pub(super) struct Sections {
    owners:   BTreeMap<String, String>,
    anchors:  BTreeMap<String, String>,
    sidebars: BTreeMap<String, Tree>,
}
impl Sections {
    pub fn read(primary: &Tree, catalog: &Catalog) -> Result<Self> {
        let mut configured = BTreeMap::new(); // bounded: primary tree has at most 512 entries.
        for (id, entry) in &primary.entries {
            if let Some(page) = &entry.page {
                let mut root = id.as_str();
                while let Some(parent) = &primary.entries[root].parent {
                    root = parent;
                }
                ensure!(
                    configured.insert(page.clone(), root.to_owned()).is_none(),
                    "section navigation assigns a page more than once: {page}"
                );
            }
        }
        let mut owners = BTreeMap::new(); // bounded: selected website page budget.
        let mut anchors = BTreeMap::new();
        let mut sidebars = BTreeMap::new();
        for page in catalog.pages.values().filter(|p| !p.asset) {
            let mut current = Some(page);
            let mut owner: Option<String> = None;
            let mut anchor = None;
            let mut depth = 0;
            while let Some(candidate) = current {
                depth += 1;
                ensure!(
                    depth <= 32,
                    "section page ancestry exceeds depth budget"
                );
                if let Some(section) = configured.get(candidate.id.as_str()) {
                    ensure!(
                        owner.as_ref().is_none_or(|prior| prior == section),
                        "page ancestry crosses navigation sections: {}",
                        page.id
                    );
                    owner = Some(section.clone());
                    anchor = Some(candidate.id.as_str().to_owned());
                }
                current = candidate
                    .parent
                    .as_ref()
                    .map(|parent| catalog.page(parent))
                    .transpose()?;
            }
            let owner = owner.ok_or_else(|| {
                anyhow::anyhow!(
                    "page has no primary navigation section: {}",
                    page.id
                )
            })?;
            anchors.insert(page.id.as_str().to_owned(), anchor.unwrap());
            owners.insert(page.id.as_str().to_owned(), owner.clone());
            if page.hidden && !configured.contains_key(page.id.as_str()) {
                continue;
            }
            let tree = sidebars.entry(owner.clone()).or_insert_with(|| {
                Tree {
                    title:    primary.entries[&owner].label.clone(),
                    entries:  BTreeMap::new(),
                    children: BTreeMap::new(),
                }
            });
            tree.entries.insert(page.id.as_str().to_owned(), Entry {
                label:  page.title.clone(),
                parent: page.parent.as_ref().map(|p| p.as_str().to_owned()),
                order:  page.order,
                href:   Some(SiteLink::page(page.route.clone())),
                page:   Some(page.id.as_str().to_owned()),
            });
        }
        for (owner, tree) in &mut sidebars {
            let ids: Vec<_> = tree.entries.keys().cloned().collect(); // bounded: selected page budget.
            for id in ids {
                let parent = tree.entries[&id]
                    .parent
                    .clone()
                    .filter(|parent| tree.entries.contains_key(parent));
                tree.entries.get_mut(&id).unwrap().parent = parent.clone();
                tree.children.entry(parent).or_default().push(id);
            }
            // The primary tab already links to a sole section home. Promote
            // its children rather than repeating that destination as a branch.
            if let Some(roots) = tree.children.get(&None) &&
                roots.len() == 1 &&
                primary.destination(owner).page.as_deref() ==
                    Some(roots[0].as_str())
            {
                let root = roots[0].clone();
                let children = tree
                    .children
                    .remove(&Some(root.clone()))
                    .unwrap_or_default();
                tree.entries.remove(&root);
                for child in &children {
                    tree.entries.get_mut(child).unwrap().parent = None;
                }
                tree.children.insert(None, children);
            }
            for children in tree.children.values_mut() {
                children.sort_by_key(|id| {
                    (
                        tree.entries[id].order,
                        tree.entries[id].label.clone(),
                        id.clone(),
                    )
                });
            }
        }
        Ok(Self { owners, anchors, sidebars })
    }

    pub fn anchor<'a>(&'a self, page: &'a str) -> &'a str {
        self.anchors.get(page).map(String::as_str).unwrap_or(page)
    }

    pub fn sidebar(&self, page: &str) -> Option<&Tree> {
        self.owners
            .get(page)
            .and_then(|owner| self.sidebars.get(owner))
            .filter(|tree| !tree.entries.is_empty())
    }
}
