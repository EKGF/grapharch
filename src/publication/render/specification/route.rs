//! Readable page routes of a paginated specification. The home page is the
//! publication root's `index.html`; every other page is the `index.html` of
//! one directory named by its page slug. Links between pages, and authored
//! references relative to the publication root, are written relative to the
//! page that contains them, so the output works beneath any mount point.
use {
    crate::publication::href::Href,
    anyhow::{Result, bail, ensure},
    std::fmt,
};

/// Longest accepted page slug, in bytes.
const MAX_SLUG_BYTES: usize = 128;
/// Directory names the publication root already owns.
const RESERVED: [&str; 1] = ["assets"];
const PAGE_FILE: &str = "index.html";

/// One path segment naming a specification page: lowercase ASCII letters and
/// digits, in runs separated by single `-` or `_` characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::publication::render) struct PageSlug(String);

impl PageSlug {
    /// Accept an explicitly declared slug, such as an authored
    /// `ga:pageSlug` or an ontology's bound namespace prefix, unchanged.
    pub(in crate::publication::render) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() &&
                value.len() <= MAX_SLUG_BYTES &&
                value.split(['-', '_']).all(|run| {
                    !run.is_empty() &&
                        run.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit()
                        })
                }),
            "page slug {value:?} must be 1 to {MAX_SLUG_BYTES} lowercase \
             ASCII letters and digits in runs separated by single '-' or '_'"
        );
        ensure!(
            !RESERVED.contains(&value),
            "page slug {value:?} is reserved by the publication root"
        );
        Ok(Self(value.into()))
    }

    /// Derive a slug from a page title: ASCII letters and digits are kept in
    /// lowercase, and every other run of characters becomes one `-`.
    pub(in crate::publication::render) fn from_title(
        title: &str,
    ) -> Result<Self> {
        let mut slug = String::new();
        for character in title.chars() {
            if character.is_ascii_alphanumeric() {
                if slug.len() == MAX_SLUG_BYTES {
                    break;
                }
                slug.push(character.to_ascii_lowercase());
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-');
        if slug.is_empty() {
            bail!(
                "page title {title:?} has no ASCII letters or digits to name \
                 its page; declare ga:pageSlug"
            )
        }
        Self::try_new(slug)
    }
}

impl fmt::Display for PageSlug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Where one page of a paginated specification is published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::publication::render) enum PageRoute {
    Home,
    Page(PageSlug),
}

impl PageRoute {
    /// Recognize a published page path, as recorded in a publication
    /// manifest.
    pub(in crate::publication::render) fn parse(path: &str) -> Option<Self> {
        if path == PAGE_FILE {
            return Some(Self::Home);
        }
        let slug = path.strip_suffix(PAGE_FILE)?.strip_suffix('/')?;
        PageSlug::try_new(slug).ok().map(Self::Page)
    }

    /// The page file, relative to the publication root.
    pub(in crate::publication::render) fn path(&self) -> String {
        match self {
            Self::Home => PAGE_FILE.into(),
            Self::Page(slug) => format!("{slug}/{PAGE_FILE}"),
        }
    }

    /// The prefix that turns a root-relative reference into one relative to
    /// this page.
    pub(in crate::publication::render) fn root(&self) -> &'static str {
        match self {
            Self::Home => "",
            Self::Page(_) => "../",
        }
    }

    /// A link to this page from `from`.
    pub(in crate::publication::render) fn href_from(
        &self,
        from: &PageRoute,
    ) -> String {
        format!("{}{}", from.root(), self.path())
    }

    /// Rebase a root-relative reference onto this page. Fragments and
    /// absolute URLs keep their meaning on every page.
    pub(in crate::publication::render) fn rebase(
        &self,
        value: &str,
    ) -> Result<Option<String>> {
        Ok(match (self, Href::parse(value)?) {
            (Self::Page(_), Href::Relative(_)) => {
                Some(format!("{}{value}", self.root()))
            },
            _ => None,
        })
    }

    /// Rebase every root-relative `href` and `src` of an HTML fragment onto
    /// this page.
    pub(in crate::publication::render) fn rebase_html(
        &self,
        html: &str,
    ) -> Result<String> {
        if *self == Self::Home {
            return Ok(html.into());
        }
        let mut document = scraper::Html::parse_fragment(html);
        let selector = scraper::Selector::parse("[href],[src]").unwrap();
        let nodes: Vec<_> =
            document.select(&selector).map(|e| e.id()).collect(); // bounded: one capped fragment.
        for id in nodes {
            let mut node = document.tree.get_mut(id).unwrap();
            let scraper::Node::Element(element) = node.value() else {
                unreachable!()
            };
            for (name, value) in element.attrs.iter_mut() {
                if matches!(&*name.local, "href" | "src") &&
                    let Some(rebased) = self.rebase(value)?
                {
                    *value = rebased.into();
                }
            }
        }
        Ok(document.root_element().inner_html())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_slugs_are_path_segments_without_normalization() {
        assert_eq!(
            PageSlug::try_new("ekg-use-case").unwrap().to_string(),
            "ekg-use-case"
        );
        assert!(PageSlug::try_new("fibo_be").is_ok());
        for invalid in
            ["", "Core", "a--b", "-a", "a-", "a/b", "a.b", "assets", "é"]
        {
            assert!(PageSlug::try_new(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn titles_derive_lowercase_ascii_slugs_or_fail() {
        assert_eq!(
            PageSlug::from_title("Status of This Document — 2.0")
                .unwrap()
                .to_string(),
            "status-of-this-document-2-0"
        );
        assert!(PageSlug::from_title("— ✓ —").is_err());
        assert!(PageSlug::from_title("Assets").is_err());
    }

    #[test]
    fn routes_link_and_rebase_relative_to_their_page() {
        let core = PageRoute::Page(PageSlug::try_new("core").unwrap());
        let extra = PageRoute::Page(PageSlug::try_new("extra").unwrap());
        assert_eq!(core.path(), "core/index.html");
        assert_eq!(extra.href_from(&core), "../extra/index.html");
        assert_eq!(
            core.href_from(&PageRoute::Home),
            "core/index.html"
        );
        assert_eq!(PageRoute::Home.href_from(&core), "../index.html");
        assert_eq!(
            PageRoute::parse("core/index.html"),
            Some(core.clone())
        );
        assert_eq!(
            PageRoute::parse("index.html"),
            Some(PageRoute::Home)
        );
        assert_eq!(PageRoute::parse("Core/index.html"), None);
        assert_eq!(PageRoute::parse("a/b/index.html"), None);
        assert_eq!(
            core.rebase_html(
                "<a href=\"#x\">x</a><img src=\"assets/a.svg\"><a \
                 href=\"https://example.org/\">y</a>"
            )
            .unwrap(),
            "<a href=\"#x\">x</a><img src=\"../assets/a.svg\"><a \
             href=\"https://example.org/\">y</a>"
        );
    }
}
