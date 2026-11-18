//! Semantic project landing-page sections derived from ordinary Markdown
//! headings.
mod panels;
mod presentation;
mod sections;
mod timeline;

pub(super) use {
    panels::panels,
    presentation::{Presentation, icon_svg},
    sections::sections,
};
