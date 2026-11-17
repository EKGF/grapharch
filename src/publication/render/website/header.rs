//! The site-wide chrome read once and shared by every generated page.
use super::{header_links, metadata, navigation, site_chrome, theme};

pub(super) struct Header {
    pub(super) title:      String,
    pub(super) repository: String,
    pub(super) theme:      theme::Theme,
    pub(super) links:      header_links::Links,
    pub(super) metadata:   metadata::Metadata,
    pub(super) navigation: navigation::Navigations,
    pub(super) chrome:     site_chrome::Options,
    pub(super) footer:     Option<site_chrome::SiteFooter>,
}
