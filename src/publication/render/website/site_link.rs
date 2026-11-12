//! Authored website link targets, independent of where the site is deployed.
//!
//! An absolute URL renders unchanged. Any other authored target is relative to
//! the website root and renders behind each page's relative root prefix, so it
//! follows the hosting mount without naming it. Host-relative paths would name
//! the mount and are rejected, as are targets that could leave the website.
use {
    crate::publication::href::Href,
    anyhow::{Context, Result, bail, ensure},
};

/// Absolute URL schemes a header link may use.
pub(super) const HEADER_SCHEMES: &[&str] = &["http", "https"];
/// Absolute URL schemes a navigation entry may use.
pub(super) const NAVIGATION_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// A validated link target of a header link, navigation entry or page route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SiteLink {
    target:    String,
    placement: Placement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placement {
    /// An absolute URL with a permitted scheme.
    Absolute,
    /// A path, optionally with query and fragment, below the website root.
    Root,
    /// A fragment of the current page.
    Fragment,
}

impl SiteLink {
    /// A website page route, root-relative by construction.
    pub fn page(route: String) -> Self {
        Self { target: route, placement: Placement::Root }
    }

    /// Validates an authored target of a link `kind` permitting `schemes`.
    pub fn authored(value: &str, schemes: &[&str], kind: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() && !value.chars().any(char::is_control),
            "{kind} URL is empty or contains control characters"
        );
        let placement = match Href::parse(value)
            .with_context(|| format!("{kind} URL {value} is invalid"))?
        {
            Href::Absolute(url) => {
                ensure!(
                    schemes.contains(&url.scheme()),
                    "{kind} URL {value} must use {} or be relative to the \
                     website root",
                    schemes.join(", ")
                );
                Placement::Absolute
            },
            Href::Fragment => Placement::Fragment,
            Href::HostRelative => {
                bail!(
                    "{kind} URL {value} depends on the host's mount point; \
                     use a URL relative to the website root"
                )
            },
            Href::Relative(relative) => {
                let path = value.split(['?', '#']).next().unwrap_or_default();
                ensure!(
                    !value.contains('\\') &&
                        !path.split('/').any(|segment| segment == ".."),
                    "{kind} URL {value} must be relative to the website root, \
                     without a backslash or parent segment"
                );
                // Resolution also catches percent-encoded parent segments.
                ensure!(
                    relative.published_file("index.html")?.is_some(),
                    "{kind} URL {value} resolves outside the website root"
                );
                Placement::Root
            },
        };
        Ok(Self { target: value.into(), placement })
    }

    /// The `href` from a page whose path to the website root is `prefix`.
    pub fn href(&self, prefix: &str) -> String {
        match self.placement {
            Placement::Root => format!("{prefix}{}", self.target),
            Placement::Absolute | Placement::Fragment => self.target.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HEADER_SCHEMES, NAVIGATION_SCHEMES, SiteLink};

    #[test]
    fn relative_targets_render_behind_the_page_root_prefix() {
        for (value, href) in [
            ("spec/main/", "../../spec/main/"),
            (
                "spec/main/pdf/document.pdf",
                "../../spec/main/pdf/document.pdf",
            ),
            (
                "./spec/?view=all#top",
                "../.././spec/?view=all#top",
            ),
            ("#contact", "#contact"),
            ("https://example.org/a/", "https://example.org/a/"),
            (
                "mailto:team@example.org",
                "mailto:team@example.org",
            ),
        ] {
            let link =
                SiteLink::authored(value, NAVIGATION_SCHEMES, "navigation")
                    .unwrap_or_else(|error| panic!("{value}: {error:#}"));
            assert_eq!(link.href("../../"), href, "{value}");
        }
        assert_eq!(
            SiteLink::page("guide/".into()).href("../"),
            "../guide/"
        );
    }

    #[test]
    fn host_relative_escaping_and_unsafe_targets_fail() {
        for value in [
            "",
            "/spec/",
            "//cdn.example.org/",
            "\\\\cdn.example.org/",
            "spec\\main/",
            "../outside/",
            "spec/../../outside/",
            "%2e%2e/outside/",
            "spec/%2E%2E/%2e%2e/outside/",
            "javascript:alert(1)",
            "data:text/html,x",
            "http://[::1/",
            "spec/\nmain/",
        ] {
            assert!(
                SiteLink::authored(value, NAVIGATION_SCHEMES, "navigation")
                    .is_err(),
                "accepted {value:?}"
            );
        }
        assert!(
            SiteLink::authored(
                "mailto:team@example.org",
                HEADER_SCHEMES,
                "header link"
            )
            .is_err()
        );
    }
}
