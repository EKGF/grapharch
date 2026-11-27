//! Source-owned website chrome selected by the pinned profile: opt-outs for
//! generated regions, an authored footer, theme tokens and the status band.
mod flag;
mod footer;
mod footer_link;
mod footer_markdown;
mod options;
mod screen_rules;
mod screen_value;
mod status_band;
mod theme_tokens;

pub(super) use {
    footer::SiteFooter,
    options::Options,
    status_band::StatusBand,
    theme_tokens::ThemeTokens,
};
