//! Source-owned website chrome selected by the pinned profile: opt-outs for
//! generated regions, an authored footer and consumer theme colour tokens.
mod colour_tokens;
mod flag;
mod footer;
mod footer_link;
mod footer_markdown;
mod options;

pub(super) use {
    colour_tokens::ColourTokens,
    footer::SiteFooter,
    options::Options,
};
