use {
    super::super::{
        markup::html_escape as esc,
        model::{Plan, node, title},
    },
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::collections::{BTreeMap, HashMap, HashSet},
};

pub(super) struct Page {
    pub id:           NamedNode,
    pub title:        String,
    pub source:       String,
    pub route:        String,
    pub asset:        bool,
    pub binary:       bool,
    pub hidden:       bool,
    pub order:        u32,
    pub parent:       Option<NamedNode>,
    pub parent_title: Option<String>,
    pub headings:     HashSet<String>,
}
pub(super) struct Catalog {
    pub pages:      BTreeMap<String, Page>,
    pub sources:    HashMap<String, String>,
    pub routes:     HashMap<String, String>,
    pub references: HashMap<(String, String), (String, String, Option<String>)>,
}
impl Catalog {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let mut catalog = Self {
            pages:      BTreeMap::new(),
            sources:    HashMap::new(),
            routes:     HashMap::new(),
            references: HashMap::new(),
        };
        let mut rows = plan.entries().await?;
        let mut current: Option<NamedNode> = None;
        let mut page_body = String::new();
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            let fragment = node(&row["fragment"])?;
            let owner = catalog.owner(plan, fragment).await?;
            let body = plan.source.text(fragment, "body").await?;
            bytes += body.len();
            ensure!(
                bytes as u64 <=
                    crate::publication::limits::PUBLICATION_CONTENT_BYTES,
                "website exceeds 128 MiB content budget"
            );
            if current.as_ref().is_some_and(|id| id != &owner) {
                let previous = current.take().unwrap();
                let page = catalog.pages.get_mut(previous.as_str()).unwrap();
                if !page.asset {
                    page.headings = super::markdown::headings(&page_body)
                        .into_iter()
                        .map(|heading| heading.anchor)
                        .collect();
                }
                page_body.clear();
            }
            current = Some(owner.clone());
            page_body.push_str(&body);
            ensure!(
                page_body.len() <= 1024 * 1024,
                "website page exceeds 1 MiB content budget"
            );
            catalog.insert(plan, owner).await?;
        }
        ensure!(
            catalog.routes.contains_key(""),
            "website requires a home page at /"
        );
        if let Some(id) = current {
            let page = catalog.pages.get_mut(id.as_str()).unwrap();
            if !page.asset {
                page.headings = super::markdown::headings(&page_body)
                    .into_iter()
                    .map(|heading| heading.anchor)
                    .collect();
            }
        }
        let mut assets = plan.assets().await?;
        while let Some(row) = assets.next().await? {
            let asset = node(&row["asset"])?;
            ensure!(
                plan.source.has_type(asset, "BinaryAsset").await?,
                "selected plan asset is not a binary artifact"
            );
            catalog.insert(plan, asset.clone()).await?;
        }
        let owner = plan.source.pattern(
            &format!(
                "?referenceOwner {} ?ref",
                crate::publication::term("reference")
            ),
            0,
        );
        let page_owner = plan.source.pattern(
            &format!(
                "?referenceOwner {} ?owner",
                crate::publication::term("partOf")
            ),
            2,
        );
        let fields = plan.source.pattern(
            &format!(
                "?ref <http://purl.org/dc/terms/identifier> ?original ; {} \
                 ?url . OPTIONAL {{ ?ref {} ?path }} OPTIONAL {{ ?ref {} \
                 ?asset }}",
                crate::publication::term("url"),
                crate::publication::term("sourcePath"),
                crate::publication::term("asset")
            ),
            1,
        );
        let mut refs = plan
            .dataset
            .select(format!(
                "SELECT DISTINCT ?owner ?original ?url ?path ?asset WHERE {{ \
                 {{ {owner} BIND(?referenceOwner AS ?owner) }} UNION {{ \
                 {owner} {page_owner} }} {fields} }}"
            ))
            .await?;
        let mut bytes = 0usize;
        while let Some(row) = refs.next().await? {
            let owner = node(&row["owner"])?;
            if !catalog.pages.contains_key(owner.as_str()) {
                continue;
            }
            let oxrdf::Term::Literal(original) = &row["original"] else {
                anyhow::bail!("reference identifier must be literal")
            };
            let oxrdf::Term::Literal(url) = &row["url"] else {
                anyhow::bail!("reference URL must be literal")
            };
            let path = match row.get("path") {
                Some(oxrdf::Term::Literal(v)) => v.value().to_owned(),
                None => String::new(),
                _ => anyhow::bail!("reference path must be literal"),
            };
            let asset = row
                .get("asset")
                .map(node)
                .transpose()?
                .map(|id| id.as_str().to_owned());
            bytes += original.value().len() + url.value().len() + path.len();
            ensure!(
                bytes <= crate::publication::limits::REFERENCE_INDEX_BYTES &&
                    catalog.references.len() <
                        crate::publication::limits::LINK_INDEX_ENTRIES,
                "website reference index exceeds budget"
            );
            let key = (owner.as_str().into(), original.value().into());
            let value = (path, url.value().into(), asset);
            if let Some(previous) =
                catalog.references.insert(key, value.clone())
            {
                ensure!(
                    previous == value,
                    "conflicting website reference metadata"
                );
            }
        }
        // Keep only compact headings and page metadata after indexing.
        let parents: Vec<_> = catalog
            .pages
            .values()
            .filter_map(|page| {
                page.parent_title
                    .as_ref()
                    .map(|title| (page.id.clone(), title.clone()))
            })
            .collect();
        for (id, parent_title) in parents {
            let matches: Vec<_> = catalog
                .pages
                .values()
                .filter(|page| !page.asset && page.title == parent_title)
                .map(|page| page.id.clone())
                .collect();
            ensure!(
                matches.len() == 1,
                "{}: parent title {parent_title} must identify exactly one \
                 topic",
                catalog.page(&id)?.source
            );
            catalog.pages.get_mut(id.as_str()).unwrap().parent =
                Some(matches[0].clone());
        }
        for page in catalog.pages.values() {
            catalog.ancestors(&page.id)?;
        }
        let paths: Vec<_> = catalog
            .pages
            .values()
            .map(|page| {
                if page.asset {
                    page.route.clone()
                } else {
                    format!("{}index.html", page.route)
                }
            })
            .collect();
        for a in &paths {
            for b in &paths {
                ensure!(
                    a == b || !b.starts_with(&format!("{a}/")),
                    "website artifact paths overlap"
                );
            }
        }
        Ok(catalog)
    }

    async fn insert(&mut self, plan: &Plan, owner: NamedNode) -> Result<()> {
        if self.pages.contains_key(owner.as_str()) {
            return Ok(());
        }
        ensure!(
            self.pages.len() < 256,
            "website exceeds 256-page budget"
        );
        let binary = plan.source.has_type(&owner, "BinaryAsset").await?;
        let asset = binary || plan.source.has_type(&owner, "TextAsset").await?;
        ensure!(
            asset || plan.source.has_type(&owner, "Page").await?,
            "selected website content has no Page or TextAsset owner"
        );
        let source = plan.source.text(&owner, "sourcePath").await?;
        let route = plan.source.text(&owner, "route").await?;
        let artifact = if asset {
            route.clone()
        } else {
            format!("{route}index.html")
        };
        super::super::files::validate_artifact_path(&artifact)?;
        ensure!(
            !matches!(
                artifact.as_str(),
                "style.css" | "manifest.yaml" | "404.html" | ".nojekyll"
            ),
            "website route collides with reserved artifact {artifact}"
        );
        ensure!(
            self.sources
                .insert(source.clone(), owner.as_str().into())
                .is_none(),
            "duplicate website source {source}"
        );
        ensure!(
            self.routes
                .insert(route.clone(), owner.as_str().into())
                .is_none(),
            "duplicate website route {route}"
        );
        let (hidden, order, parent_title) = if asset {
            (true, 999, None)
        } else {
            let hidden = match plan
                .source
                .text(&owner, "navigationHidden")
                .await?
                .as_str()
            {
                "true" => true,
                "false" => false,
                _ => anyhow::bail!("invalid navigationHidden"),
            };
            let order =
                plan.source.text(&owner, "navigationOrder").await?.parse()?;
            let parent_title = plan
                .source
                .optional(&owner, "parentTitle")
                .await?
                .map(|value| {
                    match value {
                        oxrdf::Term::Literal(value) => {
                            Ok(value.value().to_owned())
                        },
                        _ => anyhow::bail!("parentTitle must be a literal"),
                    }
                })
                .transpose()?;
            (hidden, order, parent_title)
        };
        self.pages.insert(owner.as_str().into(), Page {
            title: title(&plan.source, &owner).await?,
            id: owner,
            source,
            route,
            asset,
            binary,
            hidden,
            order,
            parent: None,
            parent_title,
            headings: HashSet::new(),
        });
        Ok(())
    }

    pub async fn owner(
        &self,
        plan: &Plan,
        fragment: &NamedNode,
    ) -> Result<NamedNode> {
        if plan.source.has_type(fragment, "TextAsset").await? {
            Ok(fragment.clone())
        } else {
            plan.source.node(fragment, "partOf").await
        }
    }

    pub fn page(&self, id: &NamedNode) -> Result<&Page> {
        self.pages
            .get(id.as_str())
            .ok_or_else(|| anyhow::anyhow!("website owner not selected: {id}"))
    }

    pub fn ancestors(&self, id: &NamedNode) -> Result<Vec<&Page>> {
        let mut result = Vec::new();
        let mut current = self.page(id)?;
        let mut visited = HashSet::from([id.clone()]);
        while let Some(parent) = &current.parent {
            ensure!(
                visited.insert(parent.clone()),
                "website navigation cycle at {parent}"
            );
            current = self.page(parent)?;
            result.push(current);
        }
        result.reverse();
        Ok(result)
    }

    fn ordered(&self, parent: Option<&NamedNode>) -> Vec<&Page> {
        let mut pages: Vec<_> = self
            .pages
            .values()
            .filter(|page| {
                !page.asset && !page.hidden && page.parent.as_ref() == parent
            })
            .collect();
        pages.sort_by(|a, b| {
            (a.order, &a.title, &a.id).cmp(&(b.order, &b.title, &b.id))
        });
        pages
    }

    pub fn navigation(
        &self,
        active: &NamedNode,
        prefix: &str,
    ) -> Result<String> {
        let ancestors: HashSet<_> = self
            .ancestors(active)?
            .iter()
            .map(|p| p.id.clone())
            .collect();
        Ok(self.nav_level(None, active, prefix, &ancestors))
    }

    fn nav_level(
        &self,
        parent: Option<&NamedNode>,
        active: &NamedNode,
        prefix: &str,
        ancestors: &HashSet<NamedNode>,
    ) -> String {
        let mut out = String::from("<ul>");
        for page in self.ordered(parent) {
            let children = self.ordered(Some(&page.id));
            let link = format!(
                "<a href=\"{prefix}{}\"{}>{}</a>",
                esc(&page.route),
                if &page.id == active {
                    " aria-current=\"page\""
                } else {
                    ""
                },
                esc(&page.title)
            );
            if children.is_empty() {
                out.push_str(&format!("<li>{link}</li>"));
            } else {
                out.push_str(&format!(
                    "<li><details \
                     data-id=\"{}\"{}><summary>{link}</summary>{}</details></\
                     li>",
                    esc(page.id.as_str()),
                    if ancestors.contains(&page.id) || &page.id == active {
                        " open"
                    } else {
                        ""
                    },
                    self.nav_level(Some(&page.id), active, prefix, ancestors)
                ));
            }
        }
        out.push_str("</ul>");
        out
    }

    pub fn previous_next(
        &self,
        id: &NamedNode,
        prefix: &str,
    ) -> Result<String> {
        let mut ordered = Vec::new();
        self.flatten(None, &mut ordered);
        let Some(index) = ordered.iter().position(|page| &page.id == id) else {
            return Ok(String::new())
        };
        let mut out = String::new();
        for (page, label) in [
            (
                index.checked_sub(1).and_then(|i| ordered.get(i)),
                "Previous",
            ),
            (ordered.get(index + 1), "Next"),
        ] {
            if let Some(page) = page {
                out.push_str(&format!(
                    "<a href=\"{prefix}{}\"><small>{label}</small>{}</a>",
                    esc(&page.route),
                    esc(&page.title)
                ));
            }
        }
        Ok(out)
    }

    fn flatten<'a>(
        &'a self,
        parent: Option<&NamedNode>,
        out: &mut Vec<&'a Page>,
    ) {
        for page in self.ordered(parent) {
            out.push(page);
            self.flatten(Some(&page.id), out);
        }
    }
}
