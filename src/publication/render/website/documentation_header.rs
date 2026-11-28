//! The header of a documentation identity skin: the brand logo, the centred
//! site title and the identity marks, then the primary tabs with the colour
//! scheme toggle and search. The repository is linked from the footer.
use super::{super::markup::html_escape as esc, header::Header};

pub(super) struct DocumentationHeader<'a> {
    pub header: &'a Header,
    pub prefix: &'a str,
    /// The primary navigation's tabs, empty when the profile omits them.
    pub tabs:   &'a str,
}

impl DocumentationHeader<'_> {
    pub(super) fn html(&self) -> String {
        let Self { header, prefix, tabs } = self;
        let search = if header.chrome.search() {
            super::search::button()
        } else {
            ""
        };
        format!(
            "<div class=\"documentation-identity-row\">{brand}<a \
             class=\"site-title\" href=\"{prefix}\">{title}</a><div \
             class=\"identity-marks\">{marks}</div></div><div \
             class=\"documentation-tabs-row\">{tabs}<div \
             class=\"site-links\">{toggle}{search}</div></div>",
            brand = header.identity.brand_html(prefix),
            title = esc(&header.title),
            marks = header.identity.marks_html(prefix),
            toggle = super::theme::COLOR_TOGGLE,
        )
    }
}
