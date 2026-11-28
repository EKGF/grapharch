//! The site-wide chrome read once and shared by every generated page.
use super::{header_links, metadata, navigation, site_chrome, theme};

pub(super) struct Header {
    pub(super) title:       String,
    pub(super) repository:  String,
    pub(super) theme:       theme::Theme,
    pub(super) identity:    site_chrome::SiteIdentity,
    pub(super) links:       header_links::Links,
    pub(super) metadata:    metadata::Metadata,
    pub(super) navigation:  navigation::Navigations,
    pub(super) chrome:      site_chrome::Options,
    pub(super) footer:      Option<site_chrome::SiteFooter>,
    pub(super) status_band: site_chrome::StatusBand,
    /// The documented software project, when the repository was discovered.
    pub(super) software:    Option<super::software::Software>,
}
