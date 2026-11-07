//! Reading order follows the first authored page occurrence in the sidebar.
use {
    super::{Navigations, Tree, esc},
    std::collections::BTreeSet,
};

impl Navigations {
    pub(in crate::publication::render::website) fn context(
        &self,
        active: &str,
        prefix: &str,
    ) -> Option<(String, String)> {
        self.trees
            .get("sidebar")
            .map(|tree| tree.context(active, prefix))
    }
}
impl Tree {
    fn reading_order<'a>(
        &'a self,
        parent: Option<&str>,
        seen: &mut BTreeSet<&'a str>,
        output: &mut Vec<&'a str>,
    ) {
        if let Some(children) = self.children.get(&parent.map(str::to_owned)) {
            for id in children {
                if let Some(page) = &self.entries[id].page {
                    if seen.insert(page) {
                        output.push(id);
                    }
                }
                self.reading_order(Some(id), seen, output);
            }
        }
    }

    fn context(&self, active: &str, prefix: &str) -> (String, String) {
        let mut ordered = Vec::new(); // bounded: tree has at most 512 entries.
        let mut seen = BTreeSet::new(); // bounded: same 512 entries.
        self.reading_order(None, &mut seen, &mut ordered);
        let Some(index) = ordered
            .iter()
            .position(|id| self.entries[*id].page.as_deref() == Some(active))
        else {
            return (String::new(), String::new());
        };
        let mut previous_next = String::new();
        for (neighbor, direction) in [
            (
                index.checked_sub(1).and_then(|i| ordered.get(i)),
                "Previous",
            ),
            (ordered.get(index + 1), "Next"),
        ] {
            if let Some(id) = neighbor {
                let entry = &self.entries[*id];
                previous_next.push_str(&format!(
                    "<a href=\"{prefix}{}\"><small>{direction}</small>{}</a>",
                    esc(entry.href.as_deref().expect("page destination")),
                    esc(&entry.label)
                ));
            }
        }
        let mut ancestors = Vec::new(); // bounded: validated depth is at most 16.
        let mut parent = self.entries[ordered[index]].parent.as_deref();
        while let Some(id) = parent {
            ancestors.push(&self.entries[id]);
            parent = self.entries[id].parent.as_deref();
        }
        let mut breadcrumbs = String::new();
        for entry in ancestors.into_iter().rev() {
            if let Some(href) = &entry.href {
                breadcrumbs.push_str(&format!(
                    "<a href=\"{}{href}\">{}</a>",
                    if entry.external { "" } else { prefix },
                    esc(&entry.label),
                    href = esc(href)
                ));
            } else {
                breadcrumbs
                    .push_str(&format!("<span>{}</span>", esc(&entry.label)));
            }
            breadcrumbs.push_str("<span aria-hidden=\"true\">/</span>");
        }
        (breadcrumbs, previous_next)
    }
}
