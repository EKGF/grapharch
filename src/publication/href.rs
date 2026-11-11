//! URL references in published HTML.
//!
//! An authored relative reference names an artifact of the same publication
//! output, such as a sibling target's PDF. It stays relative to the page that
//! contains it, so the output works wherever it is deployed: production, the
//! showcase gallery, a preview or a local build. Canonical identity such as
//! `ga:thisVersion` is metadata and never becomes the base of such a link.
//! Only a collector importing published HTML resolves relative references, and
//! then against that document's own source URL.
use {
    anyhow::{Result, bail, ensure},
    url::Url,
};

/// One `href` or `src` value, classified by how a reader resolves it.
pub(crate) enum Href<'a> {
    /// A same-document reference: `#anchor`.
    Fragment,
    /// A path-relative reference, resolved against the containing page.
    Relative(RelativeHref<'a>),
    /// A path-absolute (`/…`) or network-path (`//…`) reference. Its target
    /// depends on where the host mounts the output, not on the publication.
    HostRelative,
    /// An absolute URL.
    Absolute(Url),
}

/// A path-relative reference such as `../pdf/ekgf/document.pdf`.
pub(crate) struct RelativeHref<'a>(&'a str);

impl<'a> Href<'a> {
    pub(crate) fn parse(value: &'a str) -> Result<Self> {
        match Url::parse(value) {
            Ok(url) => Ok(Self::Absolute(url)),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                Ok(if value.starts_with('#') {
                    Self::Fragment
                } else if value.starts_with(['/', '\\']) {
                    Self::HostRelative
                } else {
                    Self::Relative(RelativeHref(value))
                })
            },
            Err(error) => bail!("invalid link {value}: {error}"),
        }
    }

    /// Accept a link in content published by this target: a same-document
    /// anchor, a portable path-relative reference, or an absolute HTTP(S) or
    /// mailto URL.
    pub(crate) fn authored(value: &'a str) -> Result<Self> {
        ensure!(
            !value.trim().is_empty() && !value.chars().any(char::is_control),
            "invalid published HTML link {value:?}"
        );
        let href = Self::parse(value)?;
        match &href {
            Self::Fragment => {},
            Self::Relative(_) => {
                ensure!(
                    !value.contains('\\'),
                    "published HTML link {value} must separate paths with /"
                )
            },
            Self::HostRelative => {
                bail!(
                    "published HTML link {value} depends on the host's mount \
                     point; use a path relative to the page"
                )
            },
            Self::Absolute(url) => {
                ensure!(
                    matches!(url.scheme(), "https" | "http" | "mailto"),
                    "unsupported published HTML link"
                )
            },
        }
        Ok(href)
    }

    pub(crate) fn is_pdf(&self) -> bool {
        match self {
            Self::Absolute(url) => url.path().ends_with(".pdf"),
            Self::Relative(relative) => relative.is_pdf(),
            Self::Fragment | Self::HostRelative => false,
        }
    }
}

impl RelativeHref<'_> {
    fn is_pdf(&self) -> bool {
        self.0
            .split(['?', '#'])
            .next()
            .is_some_and(|path| path.ends_with(".pdf"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_links_must_be_portable_or_absolute_web_links() {
        for value in ["#intro", "../pdf/omg/document.pdf", "https://ekgf.org/"]
        {
            assert!(Href::authored(value).is_ok(), "{value}");
        }
        for value in [
            "/pdf/document.pdf",
            "//example.org/document.pdf",
            "..\\document.pdf",
            "javascript:alert(1)",
            "",
        ] {
            assert!(Href::authored(value).is_err(), "{value}");
        }
    }

    #[test]
    fn pdf_references_are_recognized_with_or_without_a_scheme() {
        for value in [
            "https://example.org/a.pdf",
            "../pdf/ekgf/document.pdf",
            "document.pdf#page=3",
        ] {
            assert!(Href::parse(value).unwrap().is_pdf(), "{value}");
        }
        for value in ["#document.pdf", "index.html", "/document.pdf"] {
            assert!(!Href::parse(value).unwrap().is_pdf(), "{value}");
        }
    }
}
