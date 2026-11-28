//! The website catalog as the reference classifier's lookup.
use {
    super::{
        catalog::{Catalog, Page},
        site_registry::Destination,
    },
    crate::publication::reference::{LookupPage, ReferenceLookup},
    anyhow::Result,
    url::Url,
};

/// The catalog seen from one referring page.
pub(super) struct PageLookup<'a> {
    pub(super) page:    &'a Page,
    pub(super) catalog: &'a Catalog,
}

impl PageLookup<'_> {
    fn prefix(&self) -> String {
        "../".repeat(self.page.route.matches('/').count())
    }

    fn route_of(&self, target: &Page) -> String {
        if target.id == self.page.id {
            "#".into()
        } else {
            format!("{}{}", self.prefix(), target.route)
        }
    }

    fn target(&self, path: &str) -> Option<&Page> {
        let catalog = self.catalog;
        catalog
            .sources
            .get(path)
            .or_else(|| catalog.routes.get(path))
            .or_else(|| {
                catalog
                    .routes
                    .get(&format!("{}/", path.trim_end_matches('/')))
            })
            .and_then(|id| catalog.pages.get(id))
    }
}

impl ReferenceLookup for PageLookup<'_> {
    fn semantic(&self, value: &str) -> Result<Option<String>> {
        let (page, catalog) = (self.page, self.catalog);
        if let Some(href) = catalog
            .targets
            .resolve(value, &format!("{}index.html", page.route))?
        {
            return Ok(Some(href));
        }
        if let Some(target) =
            catalog.pages.get(value).filter(|target| !target.asset)
        {
            return Ok(Some(self.route_of(target)));
        }
        if let Some((id, anchor)) = catalog.subjects.get(value) {
            let target = &catalog.pages[id];
            return Ok(Some(
                if target.id == page.id {
                    format!("#{anchor}")
                } else {
                    format!("{}{}#{anchor}", self.prefix(), target.route)
                },
            ));
        }
        match catalog
            .references
            .get(&(page.id.as_str().into(), value.into()))
        {
            Some((_, _, Some(asset))) => {
                let target = catalog.pages.get(asset).ok_or_else(|| {
                    anyhow::anyhow!("image link asset is not selected")
                })?;
                Ok(Some(format!("{}{}", self.prefix(), target.route)))
            },
            Some((path, url, None)) if !catalog.sources.contains_key(path) => {
                Ok(Some(url.clone()))
            },
            _ => Ok(None),
        }
    }

    fn page(&self, path: &str, anchor: Option<&str>) -> Option<LookupPage> {
        let target = self.target(path)?;
        Some(LookupPage {
            route:       target.route.clone(),
            asset:       target.asset,
            has_heading: anchor.is_none_or(|anchor| {
                !target.asset && target.headings.contains(anchor)
            }),
        })
    }

    fn own_site(&self, url: &Url) -> Option<String> {
        self.catalog.bases.localize(url)
    }

    fn sibling(&self, url: &Url) -> Option<Result<String>> {
        let catalog = self.catalog;
        Some(Ok(match catalog.siblings.destination(url)? {
            Destination::Sibling(site) => site.into(),
            Destination::Local(source, anchor) => {
                let target = if source.is_empty() {
                    catalog
                        .routes
                        .get("")
                        .and_then(|id| catalog.pages.get(id))?
                } else {
                    catalog.pages.get(catalog.sources.get(&source)?)?
                };
                if let Some(anchor) = &anchor {
                    target.headings.contains(anchor.as_str()).then_some(())?;
                }
                let route = format!("{}{}", self.prefix(), target.route);
                let route = if route.is_empty() {
                    "./".to_owned()
                } else {
                    route
                };
                match anchor {
                    Some(anchor) => format!("{route}#{anchor}"),
                    None => route,
                }
            },
        }))
    }

    fn repository_file(
        &self,
        page: &str,
        link: &str,
    ) -> Option<Result<String>> {
        self.catalog
            .source_links
            .as_ref()
            .map(|links| links.href(page, link))
    }

    fn collected_image(&self, _url: &Url) -> bool { false }
}
