//! Independent target-specific navigation; pages remain content identities.
mod context;
mod sections;
use {
    super::{
        super::{
            markup::html_escape as esc,
            model::{Plan, node},
        },
        catalog::Catalog,
        site_link::{NAVIGATION_SCHEMES, SiteLink},
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};

pub(super) struct Navigations {
    trees:             BTreeMap<String, Tree>,
    sections:          Option<sections::Sections>,
    pub documentation: bool,
}
struct Tree {
    title:    String,
    entries:  BTreeMap<String, Entry>,
    children: BTreeMap<Option<String>, Vec<String>>,
}
struct Entry {
    label:  String,
    parent: Option<String>,
    order:  u32,
    href:   Option<SiteLink>,
    page:   Option<String>,
}
impl Navigations {
    pub async fn read(plan: &Plan, catalog: &Catalog) -> Result<Self> {
        let layout = optional_text(plan, &plan.profile, "layout").await?;
        ensure!(
            layout.as_deref().is_none_or(|layout| {
                matches!(layout, "documentation" | "sectioned")
            }),
            "website layout must be documentation or sectioned"
        );
        let documentation = layout.is_some();
        let mut trees = BTreeMap::new(); // bounded: three named regions.
        let mut rows = plan.source.objects(&plan.profile, "navigation").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                trees.len() < 3,
                "at most three navigation regions are supported"
            );
            let id = node(&row["value"])?;
            let region = plan.source.text(id, "navigationRegion").await?;
            ensure!(
                matches!(region.as_str(), "primary" | "sidebar" | "footer"),
                "unknown navigation region {region}"
            );
            ensure!(
                !trees.contains_key(&region),
                "duplicate navigation region {region}"
            );
            trees.insert(region, Tree::read(plan, catalog, id).await?);
        }
        let sections = if layout.as_deref() == Some("sectioned") {
            ensure!(
                !trees.contains_key("sidebar"),
                "sectioned layout derives its sidebar from page ownership; \
                 omit independent sidebar navigation"
            );
            Some(sections::Sections::read(
                trees.get("primary").ok_or_else(|| {
                    anyhow::anyhow!(
                        "sectioned layout requires primary navigation"
                    )
                })?,
                catalog,
            )?)
        } else {
            None
        };
        Ok(Self { trees, sections, documentation })
    }

    pub fn sectioned(&self) -> bool { self.sections.is_some() }

    pub fn html(
        &self,
        region: &str,
        active: &str,
        prefix: &str,
    ) -> Option<String> {
        self.html_context(region, active, active, prefix)
    }

    pub fn html_context(
        &self,
        region: &str,
        active: &str,
        owner: &str,
        prefix: &str,
    ) -> Option<String> {
        let tree = if region == "sidebar" {
            if let Some(sections) = &self.sections {
                sections.sidebar(owner)
            } else {
                self.trees.get(region)
            }
        } else {
            self.trees.get(region)
        };
        tree.map(|tree| {
            format!(
                "<nav class=\"{region}-navigation\" aria-label=\"{}\">{}</nav>",
                esc(&tree.title),
                if region == "primary" {
                    tree.tabs(
                        active,
                        self.sections
                            .as_ref()
                            .map(|sections| sections.anchor(owner))
                            .unwrap_or(owner),
                        prefix,
                    )
                } else {
                    tree.html(None, active, prefix)
                }
            )
        })
    }
}
impl Tree {
    async fn read(
        plan: &Plan,
        catalog: &Catalog,
        id: &NamedNode,
    ) -> Result<Self> {
        let title = label(plan, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("navigation requires a title"))?;
        let mut entries = BTreeMap::new(); // bounded: 512 entries per navigation.
        let mut rows = plan.source.objects(id, "navigationEntry").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                entries.len() < 512,
                "navigation exceeds 512 entries"
            );
            let entry = node(&row["value"])?;
            entries.insert(
                entry.as_str().to_owned(),
                Entry::read(plan, catalog, entry).await?,
            );
        }
        ensure!(
            !entries.is_empty(),
            "navigation must contain entries"
        );
        let mut children: BTreeMap<Option<String>, Vec<String>> =
            BTreeMap::new(); // bounded: same 512 entries.
        for (id, entry) in &entries {
            if let Some(parent) = &entry.parent {
                ensure!(
                    entries.contains_key(parent),
                    "navigation parent is outside this tree: {parent}"
                );
            }
            let mut chain = BTreeSet::new(); // bounded: maximum depth 16.
            let mut current = Some(id.as_str());
            while let Some(id) = current {
                ensure!(chain.insert(id), "navigation cycle at {id}");
                ensure!(chain.len() <= 16, "navigation exceeds depth 16");
                current =
                    entries.get(id).and_then(|entry| entry.parent.as_deref());
            }
            children
                .entry(entry.parent.clone())
                .or_default()
                .push(id.clone());
        }
        for siblings in children.values_mut() {
            siblings.sort_by_key(|id| entries[id].order);
            for pair in siblings.windows(2) {
                ensure!(
                    entries[&pair[0]].order != entries[&pair[1]].order,
                    "navigation siblings require distinct orders"
                );
            }
        }
        for (id, entry) in &entries {
            ensure!(
                entry.href.is_some() ||
                    children.contains_key(&Some(id.clone())),
                "navigation section without a destination requires children"
            );
        }
        Ok(Self { title, entries, children })
    }

    // A primary section selects its first authored descendant when it has no
    // overview page. The full tree remains available for current-section state.
    fn destination(&self, id: &str) -> &Entry {
        let entry = &self.entries[id];
        if entry.href.is_some() {
            return entry;
        }
        self.destination(&self.children[&Some(id.to_owned())][0])
    }

    fn tabs(&self, active: &str, section: &str, prefix: &str) -> String {
        let mut html = String::from("<ul>");
        if let Some(roots) = self.children.get(&None) {
            for id in roots {
                let entry = &self.entries[id];
                let destination = self.destination(id);
                html.push_str(&format!(
                    "<li><a href=\"{}\"{}{}>{}</a></li>",
                    esc(&destination
                        .href
                        .as_ref()
                        .expect("validated destination")
                        .href(prefix)),
                    if self.active(id, section) {
                        " class=\"active-section\""
                    } else {
                        ""
                    },
                    if destination.page.as_deref() == Some(active) {
                        " aria-current=\"page\""
                    } else if self.active(id, section) {
                        " aria-current=\"location\""
                    } else {
                        ""
                    },
                    esc(&entry.label),
                ));
            }
        }
        html.push_str("</ul>");
        html
    }

    fn active(&self, id: &str, page: &str) -> bool {
        self.entries[id].page.as_deref() == Some(page) ||
            self.children.get(&Some(id.into())).is_some_and(|children| {
                children.iter().any(|child| self.active(child, page))
            })
    }

    fn html(&self, parent: Option<&str>, active: &str, prefix: &str) -> String {
        let mut html = String::from("<ul>");
        if let Some(children) = self.children.get(&parent.map(str::to_owned)) {
            for id in children {
                let entry = &self.entries[id];
                let current = entry.page.as_deref() == Some(active);
                let content = if let Some(href) = &entry.href {
                    format!(
                        "<a href=\"{}\"{}>{}</a>",
                        esc(&href.href(prefix)),
                        if current {
                            " aria-current=\"page\""
                        } else {
                            ""
                        },
                        esc(&entry.label)
                    )
                } else {
                    format!("<span>{}</span>", esc(&entry.label))
                };
                if self.children.contains_key(&Some(id.clone())) {
                    html.push_str(&format!(
                        "<li><details \
                         data-id=\"{}\"{}><summary>{content}</summary>{}</\
                         details></li>",
                        esc(id),
                        if self.active(id, active) { " open" } else { "" },
                        self.html(Some(id), active, prefix)
                    ));
                } else {
                    html.push_str(&format!("<li>{content}</li>"));
                }
            }
        }
        html.push_str("</ul>");
        html
    }
}
impl Entry {
    async fn read(
        plan: &Plan,
        catalog: &Catalog,
        id: &NamedNode,
    ) -> Result<Self> {
        let page = plan.source.optional(id, "page").await?;
        let url = plan.source.optional(id, "url").await?;
        ensure!(
            page.is_none() || url.is_none(),
            "navigation entry cannot specify both page and URL"
        );
        let anchor = optional_text(plan, id, "headingAnchor").await?;
        let explicit_label = label(plan, id).await?;
        let (href, page, label) = if let Some(value) = page {
            let page = catalog.page(node(&value)?)?;
            ensure!(
                !page.asset,
                "navigation page must not be an asset"
            );
            if let Some(anchor) = &anchor {
                ensure!(
                    page.headings.contains(anchor),
                    "navigation anchor {anchor} does not exist on {}",
                    page.id
                );
            }
            (
                Some(SiteLink::page(format!(
                    "{}{}",
                    page.route,
                    anchor.map(|a| format!("#{a}")).unwrap_or_default()
                ))),
                Some(page.id.as_str().into()),
                explicit_label.unwrap_or_else(|| page.title.clone()),
            )
        } else {
            ensure!(
                anchor.is_none(),
                "navigation anchor requires a page"
            );
            let href = if let Some(value) = url {
                let Term::Literal(value) = value else {
                    anyhow::bail!("navigation URL must be a literal")
                };
                // A registered target's IRI resolves to its entry file, so a
                // PDF entry follows the build's edition name.
                let value = match catalog
                    .targets
                    .resolve(value.value(), "index.html")?
                {
                    Some(resolved) => resolved,
                    None => value.value().to_owned(),
                };
                Some(SiteLink::authored(
                    &value,
                    NAVIGATION_SCHEMES,
                    "navigation",
                )?)
            } else {
                None
            };
            (
                href,
                None,
                explicit_label.ok_or_else(|| {
                    anyhow::anyhow!(
                        "navigation section or URL entry requires a title"
                    )
                })?,
            )
        };
        let parent = plan
            .source
            .optional(id, "parentEntry")
            .await?
            .map(|value| node(&value).map(|node| node.as_str().to_owned()))
            .transpose()?;
        let order = crate::publication::validation::unsigned(
            &plan.source.optional(id, "order").await?.ok_or_else(|| {
                anyhow::anyhow!("navigation entry requires order")
            })?,
            "navigation order",
        )?;
        Ok(Self { label, parent, order, href, page })
    }
}
async fn label(plan: &Plan, id: &NamedNode) -> Result<Option<String>> {
    match plan
        .source
        .optional_by(
            id,
            &NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        )
        .await?
    {
        None => Ok(None),
        Some(Term::Literal(value)) if !value.value().trim().is_empty() => {
            Ok(Some(value.value().into()))
        },
        _ => anyhow::bail!("navigation title must be a nonempty literal"),
    }
}
async fn optional_text(
    plan: &Plan,
    id: &NamedNode,
    field: &str,
) -> Result<Option<String>> {
    match plan.source.optional(id, field).await? {
        None => Ok(None),
        Some(Term::Literal(value)) => Ok(Some(value.value().into())),
        _ => anyhow::bail!("{field} must be a literal"),
    }
}
