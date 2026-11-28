//! Source-owned website chrome selected by the pinned profile: opt-outs for
//! generated regions, an authored footer, theme tokens and the status band.
mod brand_mark;
mod chrome_sources;
mod flag;
mod footer;
mod footer_license;
mod footer_link;
mod footer_markdown;
mod identity;
mod mark_image;
mod options;
mod screen_rules;
mod screen_value;
mod social;
mod status_band;
mod theme_tokens;

pub(super) use {
    chrome_sources::ChromeSources,
    footer::SiteFooter,
    identity::SiteIdentity,
    options::Options,
    status_band::StatusBand,
    theme_tokens::ThemeTokens,
};
