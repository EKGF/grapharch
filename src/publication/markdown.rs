//! Shared GitHub-flavoured Markdown parsing options for authored documentation.
use pulldown_cmark::Options;

pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES |
        Options::ENABLE_STRIKETHROUGH |
        Options::ENABLE_TASKLISTS |
        Options::ENABLE_GFM
}
